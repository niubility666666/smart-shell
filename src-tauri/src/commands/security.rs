use crate::models::{CommandAudit, SshProfile};

const HOST_KEYRING_SERVICE: &str = "cathup-ssh";
const HOST_KEYRING_ACCOUNT: &str = "hosts";

fn hosts_keyring_entry() -> Result<keyring::Entry, String> {
    keyring::Entry::new(HOST_KEYRING_SERVICE, HOST_KEYRING_ACCOUNT)
        .map_err(|err| format!("Init keychain entry failed: {}", err))
}

#[tauri::command]
pub async fn save_hosts_secure(hosts: Vec<SshProfile>) -> Result<String, String> {
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
pub async fn load_hosts_secure() -> Result<Vec<SshProfile>, String> {
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
pub async fn clear_hosts_secure() -> Result<String, String> {
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
pub fn audit_shell_command(command: String) -> Result<CommandAudit, String> {
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

    if high_risk_patterns
        .iter()
        .any(|pattern| normalized.contains(pattern))
    {
        return Ok(CommandAudit {
            level: "high".to_string(),
            blocked: false,
            requires_confirmation: true,
            reason: "Command may cause downtime or irreversible changes".to_string(),
            suggested: "Validate on staging and scope target explicitly".to_string(),
        });
    }

    if medium_risk_patterns
        .iter()
        .any(|pattern| normalized.contains(pattern))
    {
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
