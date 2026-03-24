use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Message {
    pub role: String,
    pub content: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thinking: Option<String>,
}

pub async fn chat_completions(messages: &[Message]) -> anyhow::Result<reqwest::Response> {
    let client = reqwest::Client::new();

    let payload = serde_json::json!({
        "model": "qwen-plus",
        "messages": messages,
        "stream": true,
        "temperature": 0.7,
        "top_p": 0.95,
        "top_k": 20,
        "prescence_penalty": 0.0,
    });

    let response = client
        .post("http://127.0.0.1:7777/v1/chat/completions")
        .header("Content-Type", "application/json")
        .body(serde_json::to_string(&payload)?)
        .send()
        .await?;

    Ok(response)
}
