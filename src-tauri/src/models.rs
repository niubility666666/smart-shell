use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SshProfile {
    pub name: String,
    pub host: String,
    pub port: u16,
    pub username: String,
    pub auth_type: String,
    pub password: Option<String>,
    pub key_path: Option<String>,
    pub passphrase: Option<String>,
    pub base_path: Option<String>,
    pub group: Option<String>,
    pub tags: Option<Vec<String>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RunCommandRequest {
    pub profile: SshProfile,
    pub command: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ListDirRequest {
    pub profile: SshProfile,
    pub path: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FileRequest {
    pub profile: SshProfile,
    pub path: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct WriteFileRequest {
    pub profile: SshProfile,
    pub path: String,
    pub content: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UploadFileRequest {
    pub profile: SshProfile,
    pub remote_path: String,
    pub content_base64: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenamePathRequest {
    pub profile: SshProfile,
    pub old_path: String,
    pub new_path: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeletePathRequest {
    pub profile: SshProfile,
    pub path: String,
    pub is_dir: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MkdirRequest {
    pub profile: SshProfile,
    pub path: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PtyInputRequest {
    pub session_id: String,
    pub input: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PtyOutputRequest {
    pub session_id: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PtyCloseRequest {
    pub session_id: String,
}

#[derive(Debug, Serialize)]
pub struct CommandResult {
    pub command: String,
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
}

#[derive(Debug, Serialize)]
pub struct RemoteEntry {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
    pub size: u64,
    pub modified: u64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadFileResponse {
    pub name: String,
    pub data_base64: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PtyOutputResponse {
    pub output: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandAudit {
    pub level: String,
    pub blocked: bool,
    pub requires_confirmation: bool,
    pub reason: String,
    pub suggested: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiConfig {
    pub provider: String,
    pub endpoint: String,
    pub model: String,
    pub api_key: Option<String>,
    pub temperature: Option<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AiChatRequest {
    pub config: AiConfig,
    pub messages: Vec<AiMessage>,
}

#[derive(Debug, Serialize)]
pub struct AiChatResponse {
    pub content: String,
}
