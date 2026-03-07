import { invoke } from "@tauri-apps/api/core";
import type { AiConfig, AiMessage, CommandResult, RemoteEntry, SshProfile } from "../types";

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

export async function chatWithAi(config: AiConfig, messages: AiMessage[]) {
  return invoke<{ content: string }>("chat_with_ai", {
    request: {
      config,
      messages
    }
  });
}
