use crate::llm::Message;
use std::sync::mpsc;

pub enum StreamEvent {
    Token(String),
    Done,
}

pub async fn stream_response(
    model: String,
    messages: Vec<Message>,
    tx: mpsc::Sender<StreamEvent>,
) -> anyhow::Result<()> {
    let client = reqwest::Client::new();

    // Convert messages to OpenAI format
    let payload = serde_json::json!({
        "model": model,
        "messages": messages
            .iter()
            .map(|m| serde_json::json!({
                "role": m.role,
                "content": m.content,
            }))
            .collect::<Vec<_>>(),
        "stream": true,
        "temperature": 0.7,
    });

    let response = client
        .post("http://127.0.0.1:7777/v1/chat/completions")
        .header("Content-Type", "application/json")
        .body(serde_json::to_string(&payload)?)
        .send()
        .await?;

    let text = response.text().await?;

    for line in text.lines() {
        if line == "data: [DONE]" {
            break;
        }

        if let Some(data) = line.strip_prefix("data: ") {
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(data) {
                if let Some(token) = json
                    .get("choices")
                    .and_then(|c| c.get(0))
                    .and_then(|c| c.get("delta"))
                    .and_then(|d| d.get("content"))
                    .and_then(|c| c.as_str())
                {
                    tx.send(StreamEvent::Token(token.to_string()))?;
                }
            }
        }
    }

    // Signal that streaming is complete
    let _ = tx.send(StreamEvent::Done);

    Ok(())
}
