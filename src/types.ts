export type AuthType = "password" | "key";

export interface SshProfile {
  name: string;
  group?: string;
  tags?: string[];
  host: string;
  port: number;
  username: string;
  authType: AuthType;
  password?: string;
  keyPath?: string;
  passphrase?: string;
  basePath?: string;
}

export interface RemoteEntry {
  name: string;
  path: string;
  is_dir: boolean;
  size: number;
  modified: number;
}

export interface CommandResult {
  command: string;
  stdout: string;
  stderr: string;
  exit_code: number;
}

export interface DownloadFileResponse {
  name: string;
  dataBase64: string;
}

export interface CommandAudit {
  level: "low" | "medium" | "high";
  blocked: boolean;
  requiresConfirmation: boolean;
  reason: string;
  suggested: string;
}

export type AiProvider = "openai" | "anthropic" | "ollama" | "openai_compatible";

export interface AiMessage {
  role: "system" | "user" | "assistant";
  content: string;
}

export interface AiConfig {
  provider: AiProvider;
  endpoint: string;
  model: string;
  apiKey?: string;
  temperature?: number;
}

export interface ModelPreset {
  id: string;
  label: string;
  systemPrompt: string;
  config: AiConfig;
}

export interface PromptTemplate {
  id: string;
  label: string;
  prompt: string;
}
