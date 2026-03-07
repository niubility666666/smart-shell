#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION, CONTENT_TYPE};
use serde::{Deserialize, Serialize};
use serde_json::json;
use ssh2::Session;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::Path;
use std::time::Duration;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SshProfile {
    name: String,
    host: String,
    port: u16,
    username: String,
    auth_type: String,
    password: Option<String>,
    key_path: Option<String>,
    passphrase: Option<String>,
    base_path: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct RunCommandRequest {
    profile: SshProfile,
    command: String,
}

#[derive(Debug, Clone, Deserialize)]
struct ListDirRequest {
    profile: SshProfile,
    path: String,
}

#[derive(Debug, Clone, Deserialize)]
struct FileRequest {
    profile: SshProfile,
    path: String,
}

#[derive(Debug, Clone, Deserialize)]
struct WriteFileRequest {
    profile: SshProfile,
    path: String,
    content: String,
}

#[derive(Debug, Serialize)]
struct CommandResult {
    command: String,
    stdout: String,
    stderr: String,
    exit_code: i32,
}

#[derive(Debug, Serialize)]
struct RemoteEntry {
    name: String,
    path: String,
    is_dir: bool,
    size: u64,
    modified: u64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AiConfig {
    provider: String,
    endpoint: String,
    model: String,
    api_key: Option<String>,
    temperature: Option<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AiMessage {
    role: String,
    content: String,
}

#[derive(Debug, Clone, Deserialize)]
struct AiChatRequest {
    config: AiConfig,
    messages: Vec<AiMessage>,
}

#[derive(Debug, Serialize)]
struct AiChatResponse {
    content: String,
}

fn connect_ssh(profile: &SshProfile) -> Result<Session, String> {
    let address = format!("{}:{}", profile.host, profile.port);
    let tcp = TcpStream::connect(&address).map_err(|err| format!("连接 {} 失败: {}", address, err))?;
    tcp.set_read_timeout(Some(Duration::from_secs(15)))
        .map_err(|err| format!("设置读超时失败: {}", err))?;
    tcp.set_write_timeout(Some(Duration::from_secs(15)))
        .map_err(|err| format!("设置写超时失败: {}", err))?;

    let mut session = Session::new().map_err(|err| format!("初始化 SSH 会话失败: {}", err))?;
    session.set_tcp_stream(tcp);
    session.handshake().map_err(|err| format!("SSH 握手失败: {}", err))?;

    match profile.auth_type.as_str() {
        "password" => {
            let password = profile
                .password
                .clone()
                .ok_or_else(|| "缺少密码，无法进行 SSH 认证".to_string())?;
            session
                .userauth_password(&profile.username, &password)
                .map_err(|err| format!("密码认证失败: {}", err))?;
        }
        "key" => {
            let key_path = profile
                .key_path
                .clone()
                .ok_or_else(|| "缺少私钥路径，无法进行 SSH 认证".to_string())?;
            let passphrase = profile.passphrase.clone().filter(|value| !value.is_empty());
            session
                .userauth_pubkey_file(
                    &profile.username,
                    None,
                    Path::new(&key_path),
                    passphrase.as_deref(),
                )
                .map_err(|err| format!("私钥认证失败: {}", err))?;
        }
        other => {
            return Err(format!("不支持的认证方式: {}", other));
        }
    }

    if !session.authenticated() {
        return Err("SSH 认证失败：服务端未接受凭据".to_string());
    }

    Ok(session)
}

#[tauri::command]
async fn test_ssh_connection(profile: SshProfile) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let _session = connect_ssh(&profile)?;
        Ok(format!(
            "连接成功: {}@{}:{}",
            profile.username, profile.host, profile.port
        ))
    })
    .await
    .map_err(|err| format!("连接线程异常: {}", err))?
}

#[tauri::command]
async fn run_remote_command(request: RunCommandRequest) -> Result<CommandResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let session = connect_ssh(&request.profile)?;
        let mut channel = session
            .channel_session()
            .map_err(|err| format!("创建命令通道失败: {}", err))?;

        channel
            .exec(&request.command)
            .map_err(|err| format!("执行命令失败: {}", err))?;

        let mut stdout_buf = Vec::new();
        channel
            .read_to_end(&mut stdout_buf)
            .map_err(|err| format!("读取标准输出失败: {}", err))?;

        let mut stderr_buf = Vec::new();
        channel
            .stderr()
            .read_to_end(&mut stderr_buf)
            .map_err(|err| format!("读取错误输出失败: {}", err))?;

        channel
            .wait_close()
            .map_err(|err| format!("关闭命令通道失败: {}", err))?;

        let exit_code = channel
            .exit_status()
            .map_err(|err| format!("获取退出码失败: {}", err))?;

        Ok(CommandResult {
            command: request.command,
            stdout: String::from_utf8_lossy(&stdout_buf).to_string(),
            stderr: String::from_utf8_lossy(&stderr_buf).to_string(),
            exit_code,
        })
    })
    .await
    .map_err(|err| format!("命令线程异常: {}", err))?
}

#[tauri::command]
async fn list_remote_dir(request: ListDirRequest) -> Result<Vec<RemoteEntry>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let session = connect_ssh(&request.profile)?;
        let sftp = session
            .sftp()
            .map_err(|err| format!("创建 SFTP 通道失败: {}", err))?;
        let target = if request.path.trim().is_empty() {
            request
                .profile
                .base_path
                .clone()
                .unwrap_or_else(|| ".".to_string())
        } else {
            request.path
        };

        let entries = sftp
            .readdir(Path::new(&target))
            .map_err(|err| format!("读取目录失败: {}", err))?;

        let mut result = Vec::with_capacity(entries.len());
        for (path, stat) in entries {
            let name = path
                .file_name()
                .and_then(|file_name| file_name.to_str())
                .unwrap_or_default()
                .to_string();

            if name.is_empty() || name == "." || name == ".." {
                continue;
            }

            let perm = stat.perm.unwrap_or(0);
            let is_dir = (perm & 0o170000) == 0o040000;

            result.push(RemoteEntry {
                name,
                path: path.to_string_lossy().to_string(),
                is_dir,
                size: stat.size.unwrap_or(0),
                modified: stat.mtime.unwrap_or(0),
            });
        }

        Ok(result)
    })
    .await
    .map_err(|err| format!("目录线程异常: {}", err))?
}

#[tauri::command]
async fn read_remote_file(request: FileRequest) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let session = connect_ssh(&request.profile)?;
        let sftp = session
            .sftp()
            .map_err(|err| format!("创建 SFTP 通道失败: {}", err))?;
        let mut remote_file = sftp
            .open(Path::new(&request.path))
            .map_err(|err| format!("打开文件失败: {}", err))?;

        let mut data = Vec::new();
        remote_file
            .read_to_end(&mut data)
            .map_err(|err| format!("读取文件失败: {}", err))?;

        Ok(String::from_utf8_lossy(&data).to_string())
    })
    .await
    .map_err(|err| format!("读取文件线程异常: {}", err))?
}

#[tauri::command]
async fn write_remote_file(request: WriteFileRequest) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let session = connect_ssh(&request.profile)?;
        let sftp = session
            .sftp()
            .map_err(|err| format!("创建 SFTP 通道失败: {}", err))?;
        let mut remote_file = sftp
            .create(Path::new(&request.path))
            .map_err(|err| format!("创建/覆盖文件失败: {}", err))?;

        remote_file
            .write_all(request.content.as_bytes())
            .map_err(|err| format!("写入文件失败: {}", err))?;

        Ok(format!("保存成功: {}", request.path))
    })
    .await
    .map_err(|err| format!("写文件线程异常: {}", err))?
}

#[tauri::command]
async fn chat_with_ai(request: AiChatRequest) -> Result<AiChatResponse, String> {
    let provider = request.config.provider.to_lowercase();
    let content = match provider.as_str() {
        "openai" | "openai_compatible" => chat_openai_compatible(&request.config, &request.messages).await?,
        "anthropic" => chat_anthropic(&request.config, &request.messages).await?,
        "ollama" => chat_ollama(&request.config, &request.messages).await?,
        other => return Err(format!("不支持的 AI 提供商: {}", other)),
    };

    Ok(AiChatResponse { content })
}

fn build_http_client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(120))
        .build()
        .map_err(|err| format!("创建 HTTP 客户端失败: {}", err))
}

async fn chat_openai_compatible(config: &AiConfig, messages: &[AiMessage]) -> Result<String, String> {
    let endpoint = if config.endpoint.trim().is_empty() {
        "https://api.openai.com/v1/chat/completions".to_string()
    } else {
        config.endpoint.clone()
    };

    if config.model.trim().is_empty() {
        return Err("模型名称不能为空".to_string());
    }

    let client = build_http_client()?;

    let mut headers = HeaderMap::new();
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));

    if let Some(api_key) = config.api_key.clone().filter(|value| !value.is_empty()) {
        let value = format!("Bearer {}", api_key);
        let auth = HeaderValue::from_str(&value).map_err(|err| format!("无效 API Key: {}", err))?;
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
        .map_err(|err| format!("调用 OpenAI 接口失败: {}", err))?;

    if !response.status().is_success() {
        let status = response.status();
        let detail = response.text().await.unwrap_or_default();
        return Err(format!("AI 接口错误 {}: {}", status, detail));
    }

    let payload: serde_json::Value = response
        .json()
        .await
        .map_err(|err| format!("解析 OpenAI 响应失败: {}", err))?;

    let content = payload
        .get("choices")
        .and_then(|choices| choices.get(0))
        .and_then(|choice| choice.get("message"))
        .and_then(|message| message.get("content"))
        .and_then(|content| content.as_str())
        .unwrap_or_default()
        .to_string();

    if content.is_empty() {
        return Err("模型返回为空".to_string());
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
        return Err("Anthropic 模型名称不能为空".to_string());
    }

    let api_key = config
        .api_key
        .clone()
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "Anthropic 需要 API Key".to_string())?;

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
        return Err("Anthropic 消息不能为空".to_string());
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
        .map_err(|err| format!("调用 Anthropic 接口失败: {}", err))?;

    if !response.status().is_success() {
        let status = response.status();
        let detail = response.text().await.unwrap_or_default();
        return Err(format!("Anthropic 接口错误 {}: {}", status, detail));
    }

    let payload: serde_json::Value = response
        .json()
        .await
        .map_err(|err| format!("解析 Anthropic 响应失败: {}", err))?;

    let content = payload
        .get("content")
        .and_then(|value| value.get(0))
        .and_then(|value| value.get("text"))
        .and_then(|value| value.as_str())
        .unwrap_or_default()
        .to_string();

    if content.is_empty() {
        return Err("Anthropic 返回为空".to_string());
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
        return Err("Ollama 模型名称不能为空".to_string());
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
        .map_err(|err| format!("调用 Ollama 接口失败: {}", err))?;

    if !response.status().is_success() {
        let status = response.status();
        let detail = response.text().await.unwrap_or_default();
        return Err(format!("Ollama 接口错误 {}: {}", status, detail));
    }

    let payload: serde_json::Value = response
        .json()
        .await
        .map_err(|err| format!("解析 Ollama 响应失败: {}", err))?;

    let content = payload
        .get("message")
        .and_then(|message| message.get("content"))
        .and_then(|value| value.as_str())
        .unwrap_or_default()
        .to_string();

    if content.is_empty() {
        return Err("Ollama 返回为空".to_string());
    }

    Ok(content)
}

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            test_ssh_connection,
            run_remote_command,
            list_remote_dir,
            read_remote_file,
            write_remote_file,
            chat_with_ai
        ])
        .run(tauri::generate_context!())
        .expect("failed to run tauri app");
}
