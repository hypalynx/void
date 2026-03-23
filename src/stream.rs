use crate::llm::Message;
use std::sync::mpsc;

pub enum StreamEvent {
    Token(String),
    Done,
}

pub async fn stream_response(
    messages: Vec<Message>,
    tx: mpsc::Sender<StreamEvent>,
) -> anyhow::Result<()> {
    let response = crate::llm::chat_completions(&messages).await?;
    let text = response.text().await?;

    for line in text.lines() {
        if line == "data: [DONE]" {
            break;
        }

        if let Some(data) = line.strip_prefix("data: ")
            && let Ok(json) = serde_json::from_str::<serde_json::Value>(data)
            && let Some(token) = json
                .get("choices")
                .and_then(|c| c.get(0))
                .and_then(|c| c.get("delta"))
                .and_then(|d| d.get("content"))
                .and_then(|c| c.as_str())
        {
            tx.send(StreamEvent::Token(token.to_string()))?;
        }
    }

    // Signal that streaming is complete
    let _ = tx.send(StreamEvent::Done);

    Ok(())
}
