use crate::types::ApiMessage;

pub async fn chat_completions(
    port: u16,
    messages: &[ApiMessage],
) -> anyhow::Result<reqwest::Response> {
    let client = reqwest::Client::new();

    let payload = serde_json::json!({
        "model": "qwen-plus",
        "messages": messages,
        "stream": true,
        "temperature": 0.7,
        "top_p": 0.95,
        "top_k": 20,
        "prescence_penalty": 0.0,
        "id_slot": -1,
        "tools": crate::tool::definitions(),
    });

    let response = client
        .post(format!("http://127.0.0.1:{}/v1/chat/completions", port))
        .header("Content-Type", "application/json")
        .body(serde_json::to_string(&payload)?)
        .send()
        .await?;

    Ok(response)
}
