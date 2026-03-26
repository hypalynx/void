use crate::stream::StreamEvent;
use serde::{Deserialize, Serialize, Serializer};
use std::sync::mpsc;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ToolCall {
    pub id: String,
    #[serde(rename = "type")]
    pub call_type: String,
    pub function: ToolFunction,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ToolFunction {
    pub name: String,
    pub arguments: String,
}

#[derive(Debug, Clone)]
pub struct ToolResultInfo {
    pub tool_call_id: String,
    pub content: String,
}

#[derive(Debug, Clone)]
pub enum Message {
    User {
        content: String,
    },
    Assistant {
        content: String,
        thinking: Option<String>,
        tool_calls: Vec<ToolCall>,
    },
    ToolResult {
        tool_call_id: String,
        content: String,
    },
}

impl Serialize for Message {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        use serde::ser::SerializeMap;
        let mut map = serializer.serialize_map(None)?;

        match self {
            Message::User { content } => {
                map.serialize_entry("role", "user")?;
                map.serialize_entry("content", content)?;
            }
            Message::Assistant {
                content,
                thinking,
                tool_calls,
            } => {
                map.serialize_entry("role", "assistant")?;
                if content.is_empty() {
                    map.serialize_entry("content", &serde_json::Value::Null)?;
                } else {
                    map.serialize_entry("content", content)?;
                }
                if let Some(t) = thinking {
                    if !t.is_empty() {
                        map.serialize_entry("thinking", t)?;
                    }
                }
                if !tool_calls.is_empty() {
                    map.serialize_entry("tool_calls", tool_calls)?;
                }
            }
            Message::ToolResult {
                tool_call_id,
                content,
            } => {
                map.serialize_entry("role", "tool")?;
                map.serialize_entry("tool_call_id", tool_call_id)?;
                map.serialize_entry("content", content)?;
            }
        }

        map.end()
    }
}

pub struct AppState {
    pub input: String,
    pub cursor: usize,
    pub clipboard: String,
    pub messages: Vec<Message>,
    pub port: u16,
    pub rx: mpsc::Receiver<StreamEvent>,
    pub tx: mpsc::Sender<StreamEvent>,
    pub waiting: bool,
    pub spinner_idx: usize,
    pub current_stream_message_idx: Option<usize>,
    pub last_draw_ms: f64,
    pub fps: f64,
    pub scroll_offset: u16,
    pub target_scroll_offset: u16,
    pub total_rendered_lines: usize,
    pub msg_area_height: u16,
}
