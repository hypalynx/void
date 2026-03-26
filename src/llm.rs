use crate::types::ApiMessage;

pub async fn chat_completions(
    port: u16,
    host: String,
    model: Option<String>,
    api_key: Option<String>,
    messages: &[ApiMessage],
) -> anyhow::Result<reqwest::Response> {
    let client = reqwest::Client::new();

    let mut payload = serde_json::json!({
        "messages": messages,
        "stream": true,
        "temperature": 0.7,
        "top_p": 0.95,
        "top_k": 20,
        "prescence_penalty": 0.0,
        "id_slot": -1,
        "tools": crate::tool::definitions(),
    });

    // Only include model if provided
    if let Some(m) = model {
        payload["model"] = serde_json::Value::String(m);
    }

    let url = format!("http://{}:{}/v1/chat/completions", host, port);
    let mut request = client
        .post(&url)
        .header("Content-Type", "application/json");

    // Add authorization header if api_key is provided
    if let Some(key) = api_key {
        request = request.header("Authorization", format!("Bearer {}", key));
    }

    let response = request
        .body(serde_json::to_string(&payload)?)
        .send()
        .await?;

    Ok(response)
}
