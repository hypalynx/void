use crate::types::{ApiMessage, ToolCall, ToolFunction, ToolResultInfo};
use futures_util::TryStreamExt;
use std::collections::HashMap;
use std::sync::mpsc;

pub enum StreamEvent {
    Token(String),
    Thinking(String),
    ToolCall(ToolCall),
    ToolExecuting(String),
    ToolsExecuted(Vec<ToolResultInfo>),
    Error(String),
    Done,
}

struct PartialToolCall {
    id: String,
    name: String,
    arguments: String,
}

pub async fn stream_response(
    messages: Vec<ApiMessage>,
    tx: mpsc::Sender<StreamEvent>,
    port: u16,
    host: String,
    model: Option<String>,
    api_key: Option<String>,
    path_prefix: Option<String>,
    system_prompt: Option<String>,
) -> anyhow::Result<()> {
    let response = match crate::llm::chat_completions(port, host, model, api_key, path_prefix, system_prompt, &messages).await {
        Ok(resp) => resp,
        Err(e) => {
            let _ = tx.send(StreamEvent::Error(format!("API Error: {}", e)));
            let _ = tx.send(StreamEvent::Done);
            return Ok(());
        }
    };
    let mut stream = response.bytes_stream();
    let mut buffer = String::new();
    let mut partial_tool_calls: HashMap<String, PartialToolCall> = HashMap::new();

    while let Ok(Some(bytes)) = stream.try_next().await {
        buffer.push_str(&String::from_utf8_lossy(&bytes));

        // Process complete lines, keep incomplete ones in buffer
        let lines: Vec<&str> = buffer.split('\n').collect();
        for line in &lines[..lines.len() - 1] {
            parse_line(line, &tx, &mut partial_tool_calls)?;
        }
        // Keep the last (possibly incomplete) line
        buffer = lines.last().unwrap_or(&"").to_string();
    }

    // Handle any remaining buffer
    if !buffer.is_empty() {
        parse_line(&buffer, &tx, &mut partial_tool_calls)?;
    }

    // Emit any remaining partial tool calls
    for (_, partial) in partial_tool_calls {
        if serde_json::from_str::<serde_json::Map<String, serde_json::Value>>(&partial.arguments)
            .is_ok()
        {
            let tool_call = ToolCall {
                id: partial.id,
                call_type: "function".to_string(),
                function: ToolFunction {
                    name: partial.name,
                    arguments: partial.arguments,
                },
            };
            let _ = tx.send(StreamEvent::ToolCall(tool_call));
        }
    }

    // Signal that streaming is complete
    let _ = tx.send(StreamEvent::Done);

    Ok(())
}

fn parse_line(
    line: &str,
    tx: &mpsc::Sender<StreamEvent>,
    partial_tool_calls: &mut HashMap<String, PartialToolCall>,
) -> anyhow::Result<()> {
    if line == "data: [DONE]" {
        return Ok(());
    }

    if let Some(data) = line.strip_prefix("data: ")
        && let Ok(json) = serde_json::from_str::<serde_json::Value>(data)
    {
        let Some(delta) = json
            .get("choices")
            .and_then(|c| c.get(0))
            .and_then(|c| c.get("delta"))
        else {
            return Ok(());
        };

        if let Some(content) = delta.get("content").and_then(|c| c.as_str()) {
            // Check for XML tool calls in content (fallback if not in thinking)
            if has_xml_tool_calls(content) {
                let xml_tool_calls = parse_xml_tool_calls(content);
                for tool_call in xml_tool_calls {
                    tx.send(StreamEvent::ToolCall(tool_call))?;
                }
                // Send stripped content (without XML) for display
                let display_content = strip_xml_tool_calls(content);
                if !display_content.trim().is_empty() {
                    tx.send(StreamEvent::Token(display_content))?;
                }
            } else {
                tx.send(StreamEvent::Token(content.to_string()))?;
            }
        }
        if let Some(thinking) = delta.get("reasoning_content").and_then(|t| t.as_str()) {
            // Check for XML tool calls in thinking (Qwen sometimes outputs them there)
            if has_xml_tool_calls(thinking) {
                let xml_tool_calls = parse_xml_tool_calls(thinking);
                for tool_call in xml_tool_calls {
                    tx.send(StreamEvent::ToolCall(tool_call))?;
                }
                // Send stripped thinking (without XML) for display
                let display_thinking = strip_xml_tool_calls(thinking);
                if !display_thinking.trim().is_empty() {
                    tx.send(StreamEvent::Thinking(display_thinking))?;
                }
            } else {
                tx.send(StreamEvent::Thinking(thinking.to_string()))?;
            }
        }

        // Handle tool calls - track by index since subsequent fragments don't have id
        if let Some(tool_calls) = delta.get("tool_calls").and_then(|tc| tc.as_array()) {
            for tool_call_json in tool_calls {
                // Track by index since subsequent fragments lack the id field
                let index_key = if let Some(idx) = tool_call_json.get("index").and_then(|i| i.as_u64()) {
                    idx.to_string()
                } else {
                    continue;
                };

                let partial = partial_tool_calls
                    .entry(index_key.clone())
                    .or_insert_with(|| PartialToolCall {
                        id: String::new(),
                        name: String::new(),
                        arguments: String::new(),
                    });

                // Set id if present (only in first fragment)
                if let Some(id) = tool_call_json.get("id").and_then(|i| i.as_str()) {
                    partial.id = id.to_string();
                }

                // Set name if present (only in first fragment)
                if let Some(name) =
                    tool_call_json
                        .get("function")
                        .and_then(|f| f.get("name"))
                        .and_then(|n| n.as_str())
                {
                    partial.name = name.to_string();
                }

                // Accumulate arguments
                if let Some(args_fragment) = tool_call_json
                    .get("function")
                    .and_then(|f| f.get("arguments"))
                    .and_then(|a| a.as_str())
                {
                    partial.arguments.push_str(args_fragment);

                    // Try to emit once we have valid JSON and both id+name
                    if let Ok(_) =
                        serde_json::from_str::<serde_json::Map<String, serde_json::Value>>(
                            &partial.arguments,
                        )
                    {
                        if !partial.id.is_empty() && !partial.name.is_empty() {
                            let tool_call = ToolCall {
                                id: partial.id.clone(),
                                call_type: "function".to_string(),
                                function: ToolFunction {
                                    name: partial.name.clone(),
                                    arguments: partial.arguments.clone(),
                                },
                            };
                            tx.send(StreamEvent::ToolCall(tool_call))?;
                            partial_tool_calls.remove(&index_key);
                        }
                    }
                }
            }
        }
    }

    Ok(())
}

/// Detect if content contains XML-formatted tool calls
fn has_xml_tool_calls(content: &str) -> bool {
    (content.contains("<tool_call>") && content.contains("</tool_call>"))
        || (content.contains("<toolcall>") && content.contains("</toolcall>"))
}

/// Remove XML tool calls from content for display
fn strip_xml_tool_calls(content: &str) -> String {
    let mut result = String::new();
    let mut in_tool_call = false;

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("<tool_call>") || trimmed.starts_with("<toolcall>") {
            in_tool_call = true;
        } else if trimmed.ends_with("</tool_call>") || trimmed.ends_with("</toolcall>") {
            in_tool_call = false;
        } else if !in_tool_call && !trimmed.is_empty() {
            result.push_str(line);
            result.push('\n');
        }
    }
    result
}

/// Parse XML-formatted tool calls like:
/// <tool_call><function=Read><parameter=filePath>README.md</parameter></function></tool_call>
/// or: <toolcall><function=Glob><parameter=pattern>**/*.rs</parameter></toolcall>
fn parse_xml_tool_calls(content: &str) -> Vec<ToolCall> {
    let mut tool_calls = Vec::new();
    let mut id_counter = 0;

    // Find all tool call blocks (both formats)
    let mut remaining = content;

    loop {
        // Try to find next tool_call block
        let start_tag = if let Some(pos) = remaining.find("<tool_call>") {
            ("<tool_call>", "</tool_call>", pos)
        } else if let Some(pos) = remaining.find("<toolcall>") {
            ("<toolcall>", "</toolcall>", pos)
        } else {
            break;
        };

        let (open, close, start_pos) = start_tag;
        let block_start = start_pos + open.len();

        if let Some(end_pos) = remaining[block_start..].find(close) {
            let block_text = &remaining[block_start..block_start + end_pos];

            // Extract function name from <function=Name>
            if let Some(func_start) = block_text.find("<function=") {
                let func_pos = func_start + "<function=".len();
                if let Some(func_end) = block_text[func_pos..].find('>') {
                    let name = block_text[func_pos..func_pos + func_end].to_string();

                    // Extract parameters from <parameter=key>value</parameter>
                    let mut arguments = serde_json::Map::new();
                    let mut param_search = block_text;

                    while let Some(param_start) = param_search.find("<parameter=") {
                        let param_pos = param_start + "<parameter=".len();
                        if let Some(param_key_end) = param_search[param_pos..].find('>') {
                            let key = param_search[param_pos..param_pos + param_key_end].to_string();
                            let value_start = param_pos + param_key_end + 1;

                            if let Some(value_end) = param_search[value_start..].find("</parameter>") {
                                let value = param_search[value_start..value_start + value_end]
                                    .trim()
                                    .to_string();
                                arguments.insert(key, serde_json::Value::String(value));

                                // Move search forward
                                param_search = &param_search[value_start + value_end..];
                            } else {
                                break;
                            }
                        } else {
                            break;
                        }
                    }

                    tool_calls.push(ToolCall {
                        id: format!("call_{}", id_counter),
                        call_type: "function".to_string(),
                        function: ToolFunction {
                            name,
                            arguments: serde_json::to_string(&arguments).unwrap_or_default(),
                        },
                    });
                    id_counter += 1;
                }
            }

            // Move to next potential block
            remaining = &remaining[block_start + end_pos + close.len()..];
        } else {
            break;
        }
    }

    tool_calls
}
