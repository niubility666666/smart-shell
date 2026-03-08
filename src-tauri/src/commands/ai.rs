use crate::models::{AiChatRequest, AiChatResponse, AiConfig, AiMessage};
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION, CONTENT_TYPE};
use serde_json::json;
use std::time::Duration;

#[tauri::command]
pub async fn chat_with_ai(request: AiChatRequest) -> Result<AiChatResponse, String> {
    let provider = request.config.provider.to_lowercase();
    let content = match provider.as_str() {
        "openai" | "openai_compatible" => {
            chat_openai_compatible(&request.config, &request.messages).await?
        }
        "anthropic" => chat_anthropic(&request.config, &request.messages).await?,
        "ollama" => chat_ollama(&request.config, &request.messages).await?,
        other => return Err(format!("Unsupported AI provider: {}", other)),
    };

    Ok(AiChatResponse { content })
}

fn build_http_client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(120))
        .build()
        .map_err(|err| format!("Create HTTP client failed: {}", err))
}

async fn chat_openai_compatible(config: &AiConfig, messages: &[AiMessage]) -> Result<String, String> {
    let endpoint = if config.endpoint.trim().is_empty() {
        "https://api.openai.com/v1/chat/completions".to_string()
    } else {
        config.endpoint.clone()
    };

    if config.model.trim().is_empty() {
        return Err("Model name is required".to_string());
    }

    let client = build_http_client()?;

    let mut headers = HeaderMap::new();
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));

    if let Some(api_key) = config.api_key.clone().filter(|value| !value.is_empty()) {
        let value = format!("Bearer {}", api_key);
        let auth = HeaderValue::from_str(&value).map_err(|err| format!("Invalid API key: {}", err))?;
        headers.insert(AUTHORIZATION, auth);
    }

    let body = json!({
        "model": config.model,
        "messages": messages,
        "temperature": config.temperature.unwrap_or(0.2)
    });

    let response = client
        .post(endpoint)
        .headers(headers)
        .json(&body)
        .send()
        .await
        .map_err(|err| format!("OpenAI request failed: {}", err))?;

    if !response.status().is_success() {
        let status = response.status();
        let detail = response.text().await.unwrap_or_default();
        return Err(format!("OpenAI HTTP {}: {}", status, detail));
    }

    let payload: serde_json::Value = response
        .json()
        .await
        .map_err(|err| format!("Parse OpenAI response failed: {}", err))?;

    let content = payload
        .get("choices")
        .and_then(|choices| choices.get(0))
        .and_then(|choice| choice.get("message"))
        .and_then(|message| message.get("content"))
        .and_then(|content| content.as_str())
        .unwrap_or_default()
        .to_string();

    if content.is_empty() {
        return Err("Model returned empty content".to_string());
    }

    Ok(content)
}

async fn chat_anthropic(config: &AiConfig, messages: &[AiMessage]) -> Result<String, String> {
    let endpoint = if config.endpoint.trim().is_empty() {
        "https://api.anthropic.com/v1/messages".to_string()
    } else {
        config.endpoint.clone()
    };

    if config.model.trim().is_empty() {
        return Err("Anthropic model name is required".to_string());
    }

    let api_key = config
        .api_key
        .clone()
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "Anthropic API key is required".to_string())?;

    let mut system_prompt = String::new();
    let mut anthropic_messages = Vec::new();

    for message in messages {
        if message.role == "system" {
            if system_prompt.is_empty() {
                system_prompt = message.content.clone();
            }
            continue;
        }

        anthropic_messages.push(json!({
            "role": message.role,
            "content": message.content
        }));
    }

    if anthropic_messages.is_empty() {
        return Err("Anthropic message list is empty".to_string());
    }

    let client = build_http_client()?;
    let response = client
        .post(endpoint)
        .header("x-api-key", api_key)
        .header("anthropic-version", "2023-06-01")
        .json(&json!({
            "model": config.model,
            "system": system_prompt,
            "messages": anthropic_messages,
            "max_tokens": 1024,
            "temperature": config.temperature.unwrap_or(0.2)
        }))
        .send()
        .await
        .map_err(|err| format!("Anthropic request failed: {}", err))?;

    if !response.status().is_success() {
        let status = response.status();
        let detail = response.text().await.unwrap_or_default();
        return Err(format!("Anthropic HTTP {}: {}", status, detail));
    }

    let payload: serde_json::Value = response
        .json()
        .await
        .map_err(|err| format!("Parse Anthropic response failed: {}", err))?;

    let content = payload
        .get("content")
        .and_then(|value| value.get(0))
        .and_then(|value| value.get("text"))
        .and_then(|value| value.as_str())
        .unwrap_or_default()
        .to_string();

    if content.is_empty() {
        return Err("Anthropic returned empty content".to_string());
    }

    Ok(content)
}

async fn chat_ollama(config: &AiConfig, messages: &[AiMessage]) -> Result<String, String> {
    let endpoint = if config.endpoint.trim().is_empty() {
        "http://127.0.0.1:11434/api/chat".to_string()
    } else {
        config.endpoint.clone()
    };

    if config.model.trim().is_empty() {
        return Err("Ollama model name is required".to_string());
    }

    let client = build_http_client()?;
    let response = client
        .post(endpoint)
        .json(&json!({
            "model": config.model,
            "messages": messages,
            "stream": false,
            "options": {
                "temperature": config.temperature.unwrap_or(0.2)
            }
        }))
        .send()
        .await
        .map_err(|err| format!("Ollama request failed: {}", err))?;

    if !response.status().is_success() {
        let status = response.status();
        let detail = response.text().await.unwrap_or_default();
        return Err(format!("Ollama HTTP {}: {}", status, detail));
    }

    let payload: serde_json::Value = response
        .json()
        .await
        .map_err(|err| format!("Parse Ollama response failed: {}", err))?;

    let content = payload
        .get("message")
        .and_then(|message| message.get("content"))
        .and_then(|value| value.as_str())
        .unwrap_or_default()
        .to_string();

    if content.is_empty() {
        return Err("Ollama returned empty content".to_string());
    }

    Ok(content)
}
