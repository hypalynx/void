use crate::stream::StreamEvent;
use serde::{Deserialize, Serialize};
use std::sync::mpsc;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Message {
    pub role: String,
    pub content: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thinking: Option<String>,
}

pub struct AppState {
    pub input: String,
    pub messages: Vec<Message>,
    pub rx: mpsc::Receiver<StreamEvent>,
    pub tx: mpsc::Sender<StreamEvent>,
    pub waiting: bool,
    pub spinner_idx: usize,
    pub current_stream_message_idx: Option<usize>,
}
