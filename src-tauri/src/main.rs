
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use base64::Engine;
use once_cell::sync::Lazy;
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION, CONTENT_TYPE};
use serde::{Deserialize, Serialize};
use serde_json::json;
use ssh2::Session;
use std::collections::HashMap;
use std::io::{ErrorKind, Read, Write};
use std::net::TcpStream;
use std::path::Path;
use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use std::time::Duration;
use uuid::Uuid;

const HOST_KEYRING_SERVICE: &str = "cathup-ssh";
const HOST_KEYRING_ACCOUNT: &str = "hosts";

#[derive(Clone)]
struct PtySessionRuntime {
    input_tx: mpsc::Sender<String>,
    shutdown_tx: mpsc::Sender<()>,
    output_buffer: Arc<Mutex<String>>,
}

static PTY_SESSIONS: Lazy<Mutex<HashMap<String, PtySessionRuntime>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

#[derive(Debug, Clone, Serialize, Deserialize)]
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
    group: Option<String>,
    tags: Option<Vec<String>>,
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

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct UploadFileRequest {
    profile: SshProfile,
    remote_path: String,
    content_base64: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RenamePathRequest {
    profile: SshProfile,
    old_path: String,
    new_path: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DeletePathRequest {
    profile: SshProfile,
    path: String,
    is_dir: bool,
}

#[derive(Debug, Clone, Deserialize)]
struct MkdirRequest {
    profile: SshProfile,
    path: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PtyInputRequest {
    session_id: String,
    input: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PtyOutputRequest {
    session_id: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PtyCloseRequest {
    session_id: String,
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

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DownloadFileResponse {
    name: String,
    data_base64: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct PtyOutputResponse {
    output: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct CommandAudit {
    level: String,
    blocked: bool,
    requires_confirmation: bool,
    reason: String,
    suggested: String,
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
    let tcp = TcpStream::connect(&address)
        .map_err(|err| format!("SSH connect failed {}: {}", address, err))?;

    tcp.set_read_timeout(Some(Duration::from_secs(15)))
        .map_err(|err| format!("Set read timeout failed: {}", err))?;
    tcp.set_write_timeout(Some(Duration::from_secs(15)))
        .map_err(|err| format!("Set write timeout failed: {}", err))?;

    let mut session = Session::new().map_err(|err| format!("Create SSH session failed: {}", err))?;
    session.set_tcp_stream(tcp);
    session
        .handshake()
        .map_err(|err| format!("SSH handshake failed: {}", err))?;

    match profile.auth_type.as_str() {
        "password" => {
            let password = profile
                .password
                .clone()
                .ok_or_else(|| "Missing password for SSH auth".to_string())?;
            session
                .userauth_password(&profile.username, &password)
                .map_err(|err| format!("Password auth failed: {}", err))?;
        }
        "key" => {
            let key_path = profile
                .key_path
                .clone()
                .ok_or_else(|| "Missing private key path for SSH auth".to_string())?;
            let passphrase = profile.passphrase.clone().filter(|value| !value.is_empty());
            session
                .userauth_pubkey_file(
                    &profile.username,
                    None,
                    Path::new(&key_path),
                    passphrase.as_deref(),
                )
                .map_err(|err| format!("Key auth failed: {}", err))?;
        }
        other => return Err(format!("Unsupported auth type: {}", other)),
    }

    if !session.authenticated() {
        return Err("SSH authentication rejected by remote".to_string());
    }

    Ok(session)
}

fn push_pty_output(output: &Arc<Mutex<String>>, text: &str) {
    if text.is_empty() {
        return;
    }

    if let Ok(mut guard) = output.lock() {
        guard.push_str(text);
    }
}
fn run_pty_loop(
    profile: SshProfile,
    input_rx: mpsc::Receiver<String>,
    shutdown_rx: mpsc::Receiver<()>,
    output: Arc<Mutex<String>>,
) {
    let mut session = match connect_ssh(&profile) {
        Ok(value) => value,
        Err(err) => {
            push_pty_output(&output, &format!("[PTY] connect failed: {}\n", err));
            return;
        }
    };

    let mut channel = match session.channel_session() {
        Ok(value) => value,
        Err(err) => {
            push_pty_output(&output, &format!("[PTY] open channel failed: {}\n", err));
            return;
        }
    };

    if let Err(err) = channel.request_pty("xterm", None, Some((160, 48, 0, 0))) {
        push_pty_output(&output, &format!("[PTY] request pty failed: {}\n", err));
        return;
    }

    if let Err(err) = channel.shell() {
        push_pty_output(&output, &format!("[PTY] start shell failed: {}\n", err));
        return;
    }

    session.set_blocking(false);
    push_pty_output(&output, "[PTY] connected\n");

    let mut stdout_buffer = [0_u8; 8192];
    let mut stderr_buffer = [0_u8; 4096];

    loop {
        if shutdown_rx.try_recv().is_ok() {
            let _ = channel.write_all(b"exit\n");
            break;
        }

        loop {
            match input_rx.try_recv() {
                Ok(input) => {
                    if let Err(err) = channel.write_all(input.as_bytes()) {
                        push_pty_output(&output, &format!("[PTY] write failed: {}\n", err));
                        break;
                    }
                    let _ = channel.flush();
                }
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => break,
            }
        }

        match channel.read(&mut stdout_buffer) {
            Ok(size) if size > 0 => {
                let chunk = String::from_utf8_lossy(&stdout_buffer[..size]).to_string();
                push_pty_output(&output, &chunk);
            }
            Ok(_) => {}
            Err(err) => {
                if err.kind() != ErrorKind::WouldBlock {
                    push_pty_output(&output, &format!("[PTY] stdout read failed: {}\n", err));
                    break;
                }
            }
        }

        match channel.stderr().read(&mut stderr_buffer) {
            Ok(size) if size > 0 => {
                let chunk = String::from_utf8_lossy(&stderr_buffer[..size]).to_string();
                push_pty_output(&output, &chunk);
            }
            Ok(_) => {}
            Err(err) => {
                if err.kind() != ErrorKind::WouldBlock {
                    push_pty_output(&output, &format!("[PTY] stderr read failed: {}\n", err));
                    break;
                }
            }
        }

        if channel.eof() {
            push_pty_output(&output, "[PTY] remote session ended\n");
            break;
        }

        thread::sleep(Duration::from_millis(35));
    }

    let _ = channel.close();
    let _ = channel.wait_close();
}

fn hosts_keyring_entry() -> Result<keyring::Entry, String> {
    keyring::Entry::new(HOST_KEYRING_SERVICE, HOST_KEYRING_ACCOUNT)
        .map_err(|err| format!("Init keychain entry failed: {}", err))
}

#[tauri::command]
async fn save_hosts_secure(hosts: Vec<SshProfile>) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let payload =
            serde_json::to_string(&hosts).map_err(|err| format!("Serialize hosts failed: {}", err))?;
        let entry = hosts_keyring_entry()?;
        entry
            .set_password(&payload)
            .map_err(|err| format!("Write keychain failed: {}", err))?;
        Ok("Hosts saved to system keychain".to_string())
    })
    .await
    .map_err(|err| format!("Save hosts worker failed: {}", err))?
}

#[tauri::command]
async fn load_hosts_secure() -> Result<Vec<SshProfile>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let entry = hosts_keyring_entry()?;
        match entry.get_password() {
            Ok(payload) => {
                let hosts: Vec<SshProfile> = serde_json::from_str(&payload)
                    .map_err(|err| format!("Parse hosts payload failed: {}", err))?;
                Ok(hosts)
            }
            Err(keyring::Error::NoEntry) => Ok(Vec::new()),
            Err(err) => Err(format!("Read keychain failed: {}", err)),
        }
    })
    .await
    .map_err(|err| format!("Load hosts worker failed: {}", err))?
}

#[tauri::command]
async fn clear_hosts_secure() -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let entry = hosts_keyring_entry()?;
        match entry.delete_password() {
            Ok(_) | Err(keyring::Error::NoEntry) => Ok("Hosts removed from system keychain".to_string()),
            Err(err) => Err(format!("Clear keychain failed: {}", err)),
        }
    })
    .await
    .map_err(|err| format!("Clear hosts worker failed: {}", err))?
}

#[tauri::command]
fn audit_shell_command(command: String) -> Result<CommandAudit, String> {
    let trimmed = command.trim();
    if trimmed.is_empty() {
        return Ok(CommandAudit {
            level: "low".to_string(),
            blocked: false,
            requires_confirmation: false,
            reason: "Command is empty".to_string(),
            suggested: "".to_string(),
        });
    }

    let normalized = trimmed.to_lowercase();

    let blocked_patterns = [
        "rm -rf /",
        "mkfs",
        "dd if=",
        ":(){:|:&};:",
        "shutdown -h now",
        "reboot -f",
        "halt -f",
    ];

    let high_risk_patterns = [
        "rm -rf",
        "fdisk",
        "parted",
        "userdel",
        "iptables -f",
        "firewall-cmd --permanent --remove",
        "chmod -r 777",
        "chown -r",
        "kill -9",
        "truncate -s 0",
        "systemctl stop",
        "curl ",
        "wget ",
    ];

    let medium_risk_patterns = [
        "git reset --hard",
        "git clean -fd",
        "docker system prune",
        "sed -i",
        "mv ",
        "cp -r",
    ];

    if blocked_patterns.iter().any(|pattern| normalized.contains(pattern)) {
        return Ok(CommandAudit {
            level: "high".to_string(),
            blocked: true,
            requires_confirmation: true,
            reason: "Command matches blocked destructive pattern".to_string(),
            suggested: "Use explicit path and verify with ls/pwd first".to_string(),
        });
    }

    if high_risk_patterns.iter().any(|pattern| normalized.contains(pattern)) {
        return Ok(CommandAudit {
            level: "high".to_string(),
            blocked: false,
            requires_confirmation: true,
            reason: "Command may cause downtime or irreversible changes".to_string(),
            suggested: "Validate on staging and scope target explicitly".to_string(),
        });
    }

    if medium_risk_patterns.iter().any(|pattern| normalized.contains(pattern)) {
        return Ok(CommandAudit {
            level: "medium".to_string(),
            blocked: false,
            requires_confirmation: true,
            reason: "Command may overwrite configs or files".to_string(),
            suggested: "Consider --dry-run or backup first".to_string(),
        });
    }

    Ok(CommandAudit {
        level: "low".to_string(),
        blocked: false,
        requires_confirmation: false,
        reason: "No high-risk rule matched".to_string(),
        suggested: "".to_string(),
    })
}

#[tauri::command]
async fn open_pty_session(profile: SshProfile) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let session_id = Uuid::new_v4().to_string();
        let (input_tx, input_rx) = mpsc::channel::<String>();
        let (shutdown_tx, shutdown_rx) = mpsc::channel::<()>();
        let output_buffer = Arc::new(Mutex::new(String::new()));

        let thread_profile = profile.clone();
        let thread_output = Arc::clone(&output_buffer);
        thread::spawn(move || run_pty_loop(thread_profile, input_rx, shutdown_rx, thread_output));

        let runtime = PtySessionRuntime {
            input_tx,
            shutdown_tx,
            output_buffer,
        };

        let mut map = PTY_SESSIONS
            .lock()
            .map_err(|_| "PTY session map lock failed".to_string())?;
        map.insert(session_id.clone(), runtime);

        Ok(session_id)
    })
    .await
    .map_err(|err| format!("Open PTY worker failed: {}", err))?
}

#[tauri::command]
fn send_pty_input(request: PtyInputRequest) -> Result<String, String> {
    let map = PTY_SESSIONS
        .lock()
        .map_err(|_| "PTY session map lock failed".to_string())?;

    let runtime = map
        .get(&request.session_id)
        .ok_or_else(|| "PTY session not found".to_string())?;

    runtime
        .input_tx
        .send(request.input)
        .map_err(|err| format!("Send PTY input failed: {}", err))?;

    Ok("ok".to_string())
}

#[tauri::command]
fn read_pty_output(request: PtyOutputRequest) -> Result<PtyOutputResponse, String> {
    let map = PTY_SESSIONS
        .lock()
        .map_err(|_| "PTY session map lock failed".to_string())?;

    let runtime = map
        .get(&request.session_id)
        .ok_or_else(|| "PTY session not found".to_string())?;

    let mut buffer = runtime
        .output_buffer
        .lock()
        .map_err(|_| "PTY output buffer lock failed".to_string())?;

    let output = buffer.clone();
    buffer.clear();

    Ok(PtyOutputResponse { output })
}

#[tauri::command]
fn close_pty_session(request: PtyCloseRequest) -> Result<String, String> {
    let mut map = PTY_SESSIONS
        .lock()
        .map_err(|_| "PTY session map lock failed".to_string())?;

    if let Some(runtime) = map.remove(&request.session_id) {
        let _ = runtime.shutdown_tx.send(());
        Ok("PTY session closed".to_string())
    } else {
        Ok("PTY session not found".to_string())
    }
}
#[tauri::command]
async fn test_ssh_connection(profile: SshProfile) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let _session = connect_ssh(&profile)?;
        Ok(format!(
            "Connected: {}@{}:{}",
            profile.username, profile.host, profile.port
        ))
    })
    .await
    .map_err(|err| format!("Connection worker failed: {}", err))?
}

#[tauri::command]
async fn run_remote_command(request: RunCommandRequest) -> Result<CommandResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let session = connect_ssh(&request.profile)?;
        let mut channel = session
            .channel_session()
            .map_err(|err| format!("Open command channel failed: {}", err))?;

        channel
            .exec(&request.command)
            .map_err(|err| format!("Execute command failed: {}", err))?;

        let mut stdout_buf = Vec::new();
        channel
            .read_to_end(&mut stdout_buf)
            .map_err(|err| format!("Read stdout failed: {}", err))?;

        let mut stderr_buf = Vec::new();
        channel
            .stderr()
            .read_to_end(&mut stderr_buf)
            .map_err(|err| format!("Read stderr failed: {}", err))?;

        channel
            .wait_close()
            .map_err(|err| format!("Wait close failed: {}", err))?;

        let exit_code = channel
            .exit_status()
            .map_err(|err| format!("Read exit code failed: {}", err))?;

        Ok(CommandResult {
            command: request.command,
            stdout: String::from_utf8_lossy(&stdout_buf).to_string(),
            stderr: String::from_utf8_lossy(&stderr_buf).to_string(),
            exit_code,
        })
    })
    .await
    .map_err(|err| format!("Command worker failed: {}", err))?
}

#[tauri::command]
async fn list_remote_dir(request: ListDirRequest) -> Result<Vec<RemoteEntry>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let session = connect_ssh(&request.profile)?;
        let sftp = session
            .sftp()
            .map_err(|err| format!("Open SFTP channel failed: {}", err))?;

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
            .map_err(|err| format!("Read remote directory failed: {}", err))?;

        let mut result = Vec::with_capacity(entries.len());
        for (path, stat) in entries {
            let name = path
                .file_name()
                .and_then(|item| item.to_str())
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
    .map_err(|err| format!("List directory worker failed: {}", err))?
}

#[tauri::command]
async fn read_remote_file(request: FileRequest) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let session = connect_ssh(&request.profile)?;
        let sftp = session
            .sftp()
            .map_err(|err| format!("Open SFTP channel failed: {}", err))?;

        let mut remote_file = sftp
            .open(Path::new(&request.path))
            .map_err(|err| format!("Open remote file failed: {}", err))?;

        let mut data = Vec::new();
        remote_file
            .read_to_end(&mut data)
            .map_err(|err| format!("Read remote file failed: {}", err))?;

        Ok(String::from_utf8_lossy(&data).to_string())
    })
    .await
    .map_err(|err| format!("Read file worker failed: {}", err))?
}

#[tauri::command]
async fn write_remote_file(request: WriteFileRequest) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let session = connect_ssh(&request.profile)?;
        let sftp = session
            .sftp()
            .map_err(|err| format!("Open SFTP channel failed: {}", err))?;

        let mut remote_file = sftp
            .create(Path::new(&request.path))
            .map_err(|err| format!("Create remote file failed: {}", err))?;

        remote_file
            .write_all(request.content.as_bytes())
            .map_err(|err| format!("Write remote file failed: {}", err))?;

        Ok(format!("Saved: {}", request.path))
    })
    .await
    .map_err(|err| format!("Write file worker failed: {}", err))?
}

#[tauri::command]
async fn upload_remote_file(request: UploadFileRequest) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let binary = BASE64_STANDARD
            .decode(request.content_base64.as_bytes())
            .map_err(|err| format!("Decode upload payload failed: {}", err))?;

        let session = connect_ssh(&request.profile)?;
        let sftp = session
            .sftp()
            .map_err(|err| format!("Open SFTP channel failed: {}", err))?;

        let mut remote_file = sftp
            .create(Path::new(&request.remote_path))
            .map_err(|err| format!("Create remote file failed: {}", err))?;

        remote_file
            .write_all(&binary)
            .map_err(|err| format!("Upload write failed: {}", err))?;

        Ok(format!("Uploaded: {}", request.remote_path))
    })
    .await
    .map_err(|err| format!("Upload worker failed: {}", err))?
}

#[tauri::command]
async fn download_remote_file(request: FileRequest) -> Result<DownloadFileResponse, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let session = connect_ssh(&request.profile)?;
        let sftp = session
            .sftp()
            .map_err(|err| format!("Open SFTP channel failed: {}", err))?;

        let mut remote_file = sftp
            .open(Path::new(&request.path))
            .map_err(|err| format!("Open remote file failed: {}", err))?;

        let mut bytes = Vec::new();
        remote_file
            .read_to_end(&mut bytes)
            .map_err(|err| format!("Download read failed: {}", err))?;

        let name = Path::new(&request.path)
            .file_name()
            .and_then(|item| item.to_str())
            .unwrap_or("download.bin")
            .to_string();

        Ok(DownloadFileResponse {
            name,
            data_base64: BASE64_STANDARD.encode(bytes),
        })
    })
    .await
    .map_err(|err| format!("Download worker failed: {}", err))?
}

#[tauri::command]
async fn rename_remote_path(request: RenamePathRequest) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let session = connect_ssh(&request.profile)?;
        let sftp = session
            .sftp()
            .map_err(|err| format!("Open SFTP channel failed: {}", err))?;

        sftp.rename(Path::new(&request.old_path), Path::new(&request.new_path), None)
            .map_err(|err| format!("Rename failed: {}", err))?;

        Ok(format!("Renamed: {} -> {}", request.old_path, request.new_path))
    })
    .await
    .map_err(|err| format!("Rename worker failed: {}", err))?
}

#[tauri::command]
async fn delete_remote_path(request: DeletePathRequest) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let session = connect_ssh(&request.profile)?;
        let sftp = session
            .sftp()
            .map_err(|err| format!("Open SFTP channel failed: {}", err))?;

        if request.is_dir {
            sftp.rmdir(Path::new(&request.path))
                .map_err(|err| format!("Delete directory failed: {}", err))?;
        } else {
            sftp.unlink(Path::new(&request.path))
                .map_err(|err| format!("Delete file failed: {}", err))?;
        }

        Ok(format!("Deleted: {}", request.path))
    })
    .await
    .map_err(|err| format!("Delete worker failed: {}", err))?
}

#[tauri::command]
async fn create_remote_dir(request: MkdirRequest) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let session = connect_ssh(&request.profile)?;
        let sftp = session
            .sftp()
            .map_err(|err| format!("Open SFTP channel failed: {}", err))?;

        sftp.mkdir(Path::new(&request.path), 0o755)
            .map_err(|err| format!("Create directory failed: {}", err))?;

        Ok(format!("Directory created: {}", request.path))
    })
    .await
    .map_err(|err| format!("Create directory worker failed: {}", err))?
}
#[tauri::command]
async fn chat_with_ai(request: AiChatRequest) -> Result<AiChatResponse, String> {
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

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            test_ssh_connection,
            run_remote_command,
            list_remote_dir,
            read_remote_file,
            write_remote_file,
            upload_remote_file,
            download_remote_file,
            rename_remote_path,
            delete_remote_path,
            create_remote_dir,
            open_pty_session,
            send_pty_input,
            read_pty_output,
            close_pty_session,
            save_hosts_secure,
            load_hosts_secure,
            clear_hosts_secure,
            audit_shell_command,
            chat_with_ai
        ])
        .run(tauri::generate_context!())
        .expect("failed to run tauri app");
}
