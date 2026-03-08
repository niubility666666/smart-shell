use crate::models::{CommandResult, RunCommandRequest, SshProfile};
use crate::ssh::connect_ssh;
use std::io::Read;

#[tauri::command]
pub async fn test_ssh_connection(profile: SshProfile) -> Result<String, String> {
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
pub async fn run_remote_command(request: RunCommandRequest) -> Result<CommandResult, String> {
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
