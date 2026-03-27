use crate::stream::StreamEvent;
use ratatui::text::Line;
use serde::{Deserialize, Serialize, Serializer};
use std::sync::mpsc;
use std::time::Instant;

/// Diff line kind: context, added, or removed
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiffLineKind {
    Context,
    Added,
    Removed,
}

/// A single line in a diff
#[derive(Debug, Clone)]
pub struct DiffLine {
    pub kind: DiffLineKind,
    pub lineno: usize,     // new lineno for Context/Added, old lineno for Removed
    pub content: String,
}

/// A hunk of diff lines
#[derive(Debug, Clone)]
pub struct DiffHunk {
    pub lines: Vec<DiffLine>,
}

/// Complete file diff with structured hunks
#[derive(Debug, Clone)]
pub struct FileDiff {
    pub path: String,
    pub hunks: Vec<DiffHunk>,
}

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
    pub tool_name: String,
    pub tool_args: String,
    pub content: String,
    pub diff: Option<FileDiff>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DisplayRole {
    User,
    Assistant,
    ToolActivity,
}

#[derive(Debug, Clone)]
pub struct DisplayMessage {
    pub role: DisplayRole,
    pub content: String,
    pub thinking: Option<String>,
    pub detail: Option<String>,
    pub diff: Option<FileDiff>,
    pub lines: Vec<Line<'static>>,
    pub thinking_lines: Vec<Line<'static>>,
    pub detail_lines: Vec<Line<'static>>,
}

#[derive(Debug, Clone)]
pub enum ApiMessage {
    System {
        content: String,
    },
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

impl Serialize for ApiMessage {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        use serde::ser::SerializeMap;
        let mut map = serializer.serialize_map(None)?;

        match self {
            ApiMessage::System { content } => {
                map.serialize_entry("role", "system")?;
                map.serialize_entry("content", content)?;
            }
            ApiMessage::User { content } => {
                map.serialize_entry("role", "user")?;
                map.serialize_entry("content", content)?;
            }
            ApiMessage::Assistant {
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
            ApiMessage::ToolResult {
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
    pub messages: Vec<DisplayMessage>,
    pub api_log: Vec<ApiMessage>,
    pub tool_status: Vec<String>,
    pub show_tool_detail: bool,
    pub port: u16,
    pub host: String,
    pub model: Option<String>,
    pub api_key: Option<String>,
    pub path_prefix: Option<String>,
    pub system_prompt: Option<String>,
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
    pub input_history: Vec<String>,
    pub history_idx: Option<usize>,
    pub history_draft: String,
    pub last_exit_press: Option<Instant>,
    pub stream_task: Option<tokio::task::JoinHandle<()>>,
}
