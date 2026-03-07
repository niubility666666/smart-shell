import { invoke } from "@tauri-apps/api/core";
import type {
  AiConfig,
  AiMessage,
  CommandAudit,
  CommandResult,
  DownloadFileResponse,
  RemoteEntry,
  SshProfile
} from "../types";

export async function testConnection(profile: SshProfile) {
  return invoke<string>("test_ssh_connection", { profile });
}

export async function runRemoteCommand(profile: SshProfile, command: string) {
  return invoke<CommandResult>("run_remote_command", {
    request: { profile, command }
  });
}

export async function listRemoteDir(profile: SshProfile, path: string) {
  return invoke<RemoteEntry[]>("list_remote_dir", {
    request: { profile, path }
  });
}

export async function readRemoteFile(profile: SshProfile, path: string) {
  return invoke<string>("read_remote_file", {
    request: { profile, path }
  });
}

export async function writeRemoteFile(profile: SshProfile, path: string, content: string) {
  return invoke<string>("write_remote_file", {
    request: { profile, path, content }
  });
}

export async function uploadRemoteFile(profile: SshProfile, remotePath: string, contentBase64: string) {
  return invoke<string>("upload_remote_file", {
    request: { profile, remotePath, contentBase64 }
  });
}

export async function downloadRemoteFile(profile: SshProfile, path: string) {
  return invoke<DownloadFileResponse>("download_remote_file", {
    request: { profile, path }
  });
}

export async function renameRemotePath(profile: SshProfile, oldPath: string, newPath: string) {
  return invoke<string>("rename_remote_path", {
    request: { profile, oldPath, newPath }
  });
}

export async function deleteRemotePath(profile: SshProfile, path: string, isDir: boolean) {
  return invoke<string>("delete_remote_path", {
    request: { profile, path, isDir }
  });
}

export async function createRemoteDir(profile: SshProfile, path: string) {
  return invoke<string>("create_remote_dir", {
    request: { profile, path }
  });
}

export async function openPtySession(profile: SshProfile) {
  return invoke<string>("open_pty_session", { profile });
}

export async function sendPtyInput(sessionId: string, input: string) {
  return invoke<string>("send_pty_input", {
    request: { sessionId, input }
  });
}

export async function readPtyOutput(sessionId: string) {
  return invoke<{ output: string }>("read_pty_output", {
    request: { sessionId }
  });
}

export async function closePtySession(sessionId: string) {
  return invoke<string>("close_pty_session", {
    request: { sessionId }
  });
}

export async function auditShellCommand(command: string) {
  return invoke<CommandAudit>("audit_shell_command", { command });
}

export async function saveHostsSecure(hosts: SshProfile[]) {
  return invoke<string>("save_hosts_secure", { hosts });
}

export async function loadHostsSecure() {
  return invoke<SshProfile[]>("load_hosts_secure");
}

export async function clearHostsSecure() {
  return invoke<string>("clear_hosts_secure");
}

export async function chatWithAi(config: AiConfig, messages: AiMessage[]) {
  return invoke<{ content: string }>("chat_with_ai", {
    request: {
      config,
      messages
    }
  });
}
