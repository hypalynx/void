use crate::types::ApiMessage;

pub async fn chat_completions(
    port: u16,
    host: String,
    model: Option<String>,
    api_key: Option<String>,
    path_prefix: Option<String>,
    messages: &[ApiMessage],
) -> anyhow::Result<reqwest::Response> {
    let client = reqwest::Client::new();

    let is_local = host == "127.0.0.1" || host == "localhost";

    // Filter out thinking from all messages - models generate their own thinking,
    // we don't send back the thinking they generated
    let filtered_messages: Vec<ApiMessage> = messages.iter().map(|msg| match msg {
        ApiMessage::Assistant { content, thinking: _, tool_calls } => {
            ApiMessage::Assistant {
                content: content.clone(),
                thinking: None,
                tool_calls: tool_calls.clone(),
            }
        }
        other => other.clone(),
    }).collect();

    let mut payload = serde_json::json!({
        "messages": filtered_messages,
        "stream": true,
        "temperature": 0.7,
        "top_p": 0.95,
        "tools": crate::tool::definitions(),
    });

    // Only include Qwen-specific parameters for local models
    if is_local {
        payload["top_k"] = serde_json::json!(20);
        payload["presence_penalty"] = serde_json::json!(0.0);
        payload["id_slot"] = serde_json::json!(-1);
    }

    // Only include model if provided
    if let Some(m) = &model {
        payload["model"] = serde_json::Value::String(m.clone());
    }

    // Use HTTPS for port 443, HTTP otherwise
    let protocol = if port == 443 { "https" } else { "http" };

    // Build path with optional prefix
    let path = if let Some(prefix) = path_prefix {
        format!("{}/v1/chat/completions", prefix)
    } else {
        "/v1/chat/completions".to_string()
    };

    let url = format!("{}://{}:{}{}", protocol, host, port, path);

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

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_else(|_| "(no body)".to_string());
        return Err(anyhow::anyhow!("HTTP {}: {}", status, body));
    }

    Ok(response)
}
