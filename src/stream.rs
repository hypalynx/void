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
) -> anyhow::Result<()> {
    let response = crate::llm::chat_completions(port, &messages).await?;
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
            tx.send(StreamEvent::Token(content.to_string()))?;
        }
        if let Some(thinking) = delta.get("reasoning_content").and_then(|t| t.as_str()) {
            tx.send(StreamEvent::Thinking(thinking.to_string()))?;
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
