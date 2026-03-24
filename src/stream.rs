use crate::types::Message;
use futures_util::TryStreamExt;
use std::sync::mpsc;

pub enum StreamEvent {
    Token(String),
    Thinking(String),
    Done,
}

pub async fn stream_response(
    messages: Vec<Message>,
    tx: mpsc::Sender<StreamEvent>,
) -> anyhow::Result<()> {
    let response = crate::llm::chat_completions(&messages).await?;
    let mut stream = response.bytes_stream();
    let mut buffer = String::new();

    while let Ok(Some(bytes)) = stream.try_next().await {
        buffer.push_str(&String::from_utf8_lossy(&bytes));

        // Process complete lines, keep incomplete ones in buffer
        let lines: Vec<&str> = buffer.split('\n').collect();
        for line in &lines[..lines.len() - 1] {
            parse_line(line, &tx)?;
        }
        // Keep the last (possibly incomplete) line
        buffer = lines.last().unwrap_or(&"").to_string();
    }

    // Handle any remaining buffer
    if !buffer.is_empty() {
        parse_line(&buffer, &tx)?;
    }

    // Signal that streaming is complete
    let _ = tx.send(StreamEvent::Done);

    Ok(())
}

fn parse_line(line: &str, tx: &mpsc::Sender<StreamEvent>) -> anyhow::Result<()> {
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
    }

    Ok(())
}
