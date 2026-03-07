
<template>
  <div class="app-shell">
    <header class="window-bar">
      <div class="window-dots">
        <button class="dot close" @click="closeWindow" aria-label="close"></button>
        <button class="dot min" @click="minimizeWindow" aria-label="minimize"></button>
        <button class="dot max" @click="toggleMaximize" aria-label="maximize"></button>
      </div>
      <div class="brand">CATHUP SSH V2.0</div>
    </header>

    <main class="layout">
      <aside class="hosts-panel">
        <div class="hosts-header">
          <div class="logo">C</div>
          <div>
            <h1>Cathup</h1>
            <p>HOSTS</p>
          </div>
        </div>

        <div class="host-tools">
          <button :disabled="hostStorageBusy" @click="persistHostsSecureManually">Sync Keychain</button>
          <button :disabled="hostStorageBusy" @click="wipeSecureHosts">Clear Keychain</button>
        </div>

        <div class="hosts-list">
          <section v-for="[groupName, items] in groupedHosts" :key="groupName" class="group-block">
            <header>{{ groupName }}</header>
            <button
              v-for="item in items"
              :key="`${item.host.host}-${item.index}`"
              :class="['host-card', { active: item.index === selectedHostIndex }]"
              @click="selectHost(item.index)"
            >
              <div>
                <strong>{{ item.host.name }}</strong>
                <small>{{ item.host.username }}@{{ item.host.host }}:{{ item.host.port }}</small>
                <div v-if="item.host.tags && item.host.tags.length > 0" class="tag-row">
                  <span v-for="tag in item.host.tags" :key="tag" class="tag-chip">{{ tag }}</span>
                </div>
              </div>
              <div class="host-actions">
                <button @pointerdown.stop @click.stop="pingHost(item.index)">Ping</button>
                <button @pointerdown.stop @click.stop="removeHost(item.index)">Delete</button>
              </div>
            </button>
          </section>
        </div>

        <button class="add-host" @click="showHostForm = !showHostForm">
          + Add New Host
        </button>

        <form v-if="showHostForm" class="host-form" @submit.prevent="saveHost">
          <input v-model="hostDraft.name" placeholder="Name" required />
          <input v-model="hostDraft.group" placeholder="Group, e.g. prod/dev" />
          <input v-model="hostTagsInput" placeholder="Tags, comma separated" />
          <input v-model="hostDraft.host" placeholder="Host / IP" required />
          <input v-model.number="hostDraft.port" type="number" placeholder="Port" required />
          <input v-model="hostDraft.username" placeholder="Username" required />
          <select v-model="hostDraft.authType">
            <option value="password">Password</option>
            <option value="key">Private Key</option>
          </select>
          <input
            v-if="hostDraft.authType === 'password'"
            v-model="hostDraft.password"
            type="password"
            placeholder="Password"
            required
          />
          <input
            v-else
            v-model="hostDraft.keyPath"
            placeholder="Key path, e.g. C:/Users/me/.ssh/id_rsa"
            required
          />
          <input v-model="hostDraft.basePath" placeholder="Default path, e.g. /home/ubuntu" />
          <button type="submit">Save Host</button>
        </form>
      </aside>

      <section class="workspace">
        <div class="workspace-header">
          <span>Hosts / <strong>{{ activeHost?.name || 'Select a host' }}</strong></span>
          <div class="tabs">
            <button :class="{ active: activeTab === 'terminal' }" @click="activeTab = 'terminal'">Terminal</button>
            <button :class="{ active: activeTab === 'files' }" @click="activeTab = 'files'">Files</button>
          </div>
        </div>

        <div v-if="activeTab === 'terminal'" class="terminal-panel">
          <div class="terminal-toolbar">
            <button :disabled="!activeHost || ptyConnected || ptyConnecting" @click="connectTerminal">
              {{ ptyConnecting ? 'Connecting...' : 'Connect PTY' }}
            </button>
            <button :disabled="!ptyConnected" @click="disconnectTerminal(false)">Disconnect</button>
            <button :disabled="!ptyConnected" @click="sendCtrlC">Ctrl+C</button>
            <button @click="clearLogs">Clear</button>
            <span :class="['pty-status', ptyConnected ? 'on' : 'off']">
              {{ ptyConnected ? 'PTY Online' : 'PTY Offline' }}
            </span>
          </div>

          <div v-if="lastAudit" :class="['audit-banner', lastAudit.level]">
            <strong>Audit: {{ lastAudit.level.toUpperCase() }}</strong>
            <span>{{ lastAudit.reason }}</span>
          </div>

          <div class="terminal-log" ref="logRef">
            <div v-for="(item, index) in logs" :key="`${item.ts}-${index}`" :class="['log-item', item.kind]">
              {{ item.text }}
            </div>
            <div v-if="!activeHost" class="empty-state">No active session</div>
          </div>

          <div class="command-row">
            <input
              v-model="commandInput"
              class="command-input"
              placeholder="Type Linux command, or drag AI response here"
              :disabled="!activeHost"
              @keydown.enter.prevent="executeCommand"
              @dragover.prevent
              @drop="onCommandDrop"
            />
            <button :disabled="!activeHost || !commandInput.trim()" @click="executeCommand">Run</button>
          </div>
        </div>
        <div v-else class="files-panel">
          <div class="file-toolbar">
            <button :disabled="!activeHost || fileBusy" @click="goUp">Up</button>
            <button :disabled="!activeHost || fileBusy" @click="refreshDir()">Refresh</button>
            <button :disabled="!activeHost || fileBusy" @click="triggerUpload">Upload</button>
            <button :disabled="!activeHost || fileBusy" @click="promptCreateFolder">New Folder</button>
            <input
              v-model="currentPath"
              :disabled="!activeHost || fileBusy"
              @keydown.enter.prevent="refreshDir(currentPath)"
            />
            <input ref="uploadInputRef" type="file" multiple class="hidden-upload" @change="onUploadInputChange" />
          </div>

          <div class="file-content">
            <div class="tree" @dragover.prevent @drop.prevent="onFileDrop">
              <button
                v-for="entry in sortedEntries"
                :key="entry.path"
                class="entry"
                @dblclick="openEntry(entry.path, entry.is_dir)"
                @contextmenu.prevent="openFileContextMenu($event, entry)"
              >
                <span>{{ entry.is_dir ? '[DIR]' : '[FILE]' }}</span>
                <span>{{ entry.name }}</span>
              </button>
              <div class="drop-tip">Drag local files here to upload</div>
            </div>

            <div class="editor">
              <div class="editor-header">
                <span>{{ selectedFilePath || 'Double click a file to edit' }}</span>
                <div class="editor-actions">
                  <button :disabled="!selectedFilePath || fileBusy" @click="downloadCurrentFile">Download</button>
                  <button :disabled="!selectedFilePath || fileBusy" @click="saveCurrentFile">Save</button>
                </div>
              </div>
              <textarea
                v-model="selectedFileContent"
                :disabled="!selectedFilePath || fileBusy"
                placeholder="File content"
              ></textarea>
            </div>
          </div>
        </div>
      </section>

      <aside class="ai-panel">
        <div class="ai-header">
          <h2>AI Terminal Assistant</h2>
          <button @click="showAiConfig = !showAiConfig">Config</button>
        </div>

        <div v-if="showAiConfig" class="ai-config">
          <label>
            Model Preset
            <select v-model="selectedPresetId" @change="applyPresetById(selectedPresetId)">
              <option v-for="preset in MODEL_PRESETS" :key="preset.id" :value="preset.id">{{ preset.label }}</option>
            </select>
          </label>

          <label>
            Prompt Template
            <select v-model="selectedPromptTemplateId" @change="applyPromptTemplate(selectedPromptTemplateId)">
              <option value="">No template</option>
              <option v-for="tmpl in PROMPT_TEMPLATES" :key="tmpl.id" :value="tmpl.id">{{ tmpl.label }}</option>
            </select>
          </label>

          <select v-model="aiConfig.provider">
            <option value="openai">OpenAI</option>
            <option value="anthropic">Anthropic</option>
            <option value="ollama">Ollama</option>
            <option value="openai_compatible">OpenAI Compatible</option>
          </select>
          <input v-model="aiConfig.endpoint" placeholder="Endpoint" />
          <input v-model="aiConfig.model" placeholder="Model" />
          <input v-model="aiConfig.apiKey" type="password" placeholder="API Key (optional for local Ollama)" />
          <label>
            Temperature
            <input v-model.number="aiConfig.temperature" type="number" min="0" max="2" step="0.1" />
          </label>
          <textarea v-model="systemPrompt" placeholder="System prompt template"></textarea>
        </div>

        <div class="messages">
          <article
            v-for="(message, index) in aiMessages"
            :key="`${message.role}-${index}`"
            :class="['bubble', message.role]"
          >
            <header>{{ message.role === 'assistant' ? 'AI' : 'You' }}</header>
            <p class="bubble-text">{{ message.content }}</p>
            <div v-if="message.role === 'assistant'" class="bubble-actions">
              <button @click="appendToCommand(message.content)">Insert to Input</button>
              <button draggable="true" @dragstart="onAiDragStart($event, message.content)">Drag Block</button>
            </div>
          </article>
        </div>

        <div class="chat-input-row">
          <textarea
            v-model="aiPrompt"
            placeholder="Ask Linux or SSH questions..."
            @keydown.ctrl.enter.prevent="sendAiMessage"
          ></textarea>
          <button :disabled="aiBusy || !aiPrompt.trim()" @click="sendAiMessage">
            {{ aiBusy ? 'Thinking...' : 'Send' }}
          </button>
        </div>
      </aside>
    </main>

    <div
      v-if="fileMenu.visible"
      class="context-menu"
      :style="{ left: `${fileMenu.x}px`, top: `${fileMenu.y}px` }"
      @pointerdown.stop
      @click.stop
    >
      <button @click="contextOpen">Open</button>
      <button @click="contextDownload">Download</button>
      <button @click="contextRename">Rename</button>
      <button @click="contextDelete">Delete</button>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { getCurrentWindow } from "@tauri-apps/api/window";
import {
  auditShellCommand,
  chatWithAi,
  clearHostsSecure,
  closePtySession,
  createRemoteDir,
  deleteRemotePath,
  downloadRemoteFile,
  listRemoteDir,
  loadHostsSecure,
  openPtySession,
  readPtyOutput,
  readRemoteFile,
  renameRemotePath,
  saveHostsSecure,
  sendPtyInput,
  testConnection,
  uploadRemoteFile,
  writeRemoteFile
} from "./composables/useBackend";
import type {
  AiConfig,
  AiMessage,
  CommandAudit,
  ModelPreset,
  PromptTemplate,
  RemoteEntry,
  SshProfile
} from "./types";
type LogKind = "cmd" | "out" | "err" | "sys" | "warn";

interface LogItem {
  kind: LogKind;
  text: string;
  ts: number;
}

interface HostItem {
  host: SshProfile;
  index: number;
}

const HOST_LEGACY_STORAGE_KEY = "cathup-hosts-v1";
const AI_STORAGE_KEY = "cathup-ai-config-v2";
const SYSTEM_PROMPT_STORAGE_KEY = "cathup-ai-system-prompt-v1";
const PRESET_STORAGE_KEY = "cathup-ai-preset-v1";

const DEFAULT_SYSTEM_PROMPT =
  "You are a Linux and SSH automation assistant. Prefer executable commands, brief explanations, and risk warnings before dangerous actions.";

const MODEL_PRESETS: ModelPreset[] = [
  {
    id: "openai-default",
    label: "OpenAI / GPT-4o-mini",
    systemPrompt: DEFAULT_SYSTEM_PROMPT,
    config: {
      provider: "openai",
      endpoint: "https://api.openai.com/v1/chat/completions",
      model: "gpt-4o-mini",
      temperature: 0.2
    }
  },
  {
    id: "anthropic-sonnet",
    label: "Anthropic / Claude Sonnet",
    systemPrompt: DEFAULT_SYSTEM_PROMPT,
    config: {
      provider: "anthropic",
      endpoint: "https://api.anthropic.com/v1/messages",
      model: "claude-3-7-sonnet-latest",
      temperature: 0.2
    }
  },
  {
    id: "ollama-qwen",
    label: "Ollama / Qwen2.5",
    systemPrompt: DEFAULT_SYSTEM_PROMPT,
    config: {
      provider: "ollama",
      endpoint: "http://127.0.0.1:11434/api/chat",
      model: "qwen2.5:latest",
      temperature: 0.2
    }
  }
];

const PROMPT_TEMPLATES: PromptTemplate[] = [
  {
    id: "cpu-troubleshooting",
    label: "CPU Troubleshooting",
    prompt: "Give me a Linux CPU spike troubleshooting checklist. Start with read-only commands, then add safe mitigation options."
  },
  {
    id: "disk-cleanup",
    label: "Disk Cleanup",
    prompt: "Suggest safe disk cleanup commands for Linux and avoid deleting system-critical files."
  },
  {
    id: "service-debug",
    label: "Service Failure Debug",
    prompt: "My service fails to start. Provide systemd and journalctl diagnosis commands with decision branches."
  }
];

const hosts = ref<SshProfile[]>([]);
const selectedHostIndex = ref<number>(-1);
const showHostForm = ref(false);
const hostTagsInput = ref("");
const hostStorageReady = ref(false);
const hostStorageBusy = ref(false);
const hostDraft = ref<SshProfile>(createEmptyHost());

const activeTab = ref<"terminal" | "files">("terminal");
const logs = ref<LogItem[]>([]);
const logRef = ref<HTMLElement | null>(null);

const commandInput = ref("");
const ptySessionId = ref("");
const ptyConnecting = ref(false);
const ptyConnected = ref(false);
const pollingPty = ref(false);
let ptyPollTimer: number | null = null;
const lastAudit = ref<CommandAudit | null>(null);

const fileBusy = ref(false);
const currentPath = ref(".");
const fileEntries = ref<RemoteEntry[]>([]);
const selectedFilePath = ref("");
const selectedFileContent = ref("");
const uploadInputRef = ref<HTMLInputElement | null>(null);

const fileMenu = ref<{ visible: boolean; x: number; y: number; entry: RemoteEntry | null }>({
  visible: false,
  x: 0,
  y: 0,
  entry: null
});

const aiBusy = ref(false);
const showAiConfig = ref(false);
const aiPrompt = ref("");
const systemPrompt = ref(DEFAULT_SYSTEM_PROMPT);
const selectedPresetId = ref("openai-default");
const selectedPromptTemplateId = ref("");
const aiMessages = ref<AiMessage[]>([
  {
    role: "assistant",
    content: "Hello. I am your SSH assistant. I support model presets, command auditing, and drag to command input."
  }
]);

const aiConfig = ref<AiConfig>({
  provider: "openai",
  endpoint: "https://api.openai.com/v1/chat/completions",
  model: "gpt-4o-mini",
  apiKey: "",
  temperature: 0.2
});

const activeHost = computed(() => hosts.value[selectedHostIndex.value] ?? null);

const groupedHosts = computed<[string, HostItem[]][]>(() => {
  const groupMap: Record<string, HostItem[]> = {};
  hosts.value.forEach((host, index) => {
    const group = host.group?.trim() || "Ungrouped";
    if (!groupMap[group]) {
      groupMap[group] = [];
    }
    groupMap[group].push({ host, index });
  });

  return Object.entries(groupMap).sort(([a], [b]) => a.localeCompare(b));
});

const sortedEntries = computed(() => {
  return [...fileEntries.value].sort((a, b) => {
    if (a.is_dir !== b.is_dir) {
      return a.is_dir ? -1 : 1;
    }
    return a.name.localeCompare(b.name);
  });
});

watch(
  logs,
  async () => {
    await nextTick();
    if (logRef.value) {
      logRef.value.scrollTop = logRef.value.scrollHeight;
    }
  },
  { deep: true }
);

watch(
  aiConfig,
  () => {
    localStorage.setItem(AI_STORAGE_KEY, JSON.stringify(aiConfig.value));
  },
  { deep: true }
);

watch(systemPrompt, () => {
  localStorage.setItem(SYSTEM_PROMPT_STORAGE_KEY, systemPrompt.value);
});

watch(selectedPresetId, () => {
  localStorage.setItem(PRESET_STORAGE_KEY, selectedPresetId.value);
});

watch(
  hosts,
  async () => {
    if (!hostStorageReady.value) {
      return;
    }
    await persistHostsSecure();
  },
  { deep: true }
);

function createEmptyHost(): SshProfile {
  return {
    name: "",
    group: "Default",
    tags: [],
    host: "",
    port: 22,
    username: "",
    authType: "password",
    password: "",
    keyPath: "",
    passphrase: "",
    basePath: "."
  };
}

function addLog(kind: LogKind, text: string) {
  logs.value.push({ kind, text, ts: Date.now() });
}
async function persistHostsSecure() {
  if (hostStorageBusy.value) {
    return;
  }

  hostStorageBusy.value = true;
  try {
    await saveHostsSecure(hosts.value);
  } catch (error) {
    addLog("err", `Keychain save failed: ${String(error)}`);
  } finally {
    hostStorageBusy.value = false;
  }
}

async function persistHostsSecureManually() {
  await persistHostsSecure();
  addLog("sys", "Host config synced to system keychain");
}

async function wipeSecureHosts() {
  const ok = window.confirm("Clear host records in system keychain?");
  if (!ok) {
    return;
  }

  try {
    await clearHostsSecure();
    hosts.value = [];
    selectedHostIndex.value = -1;
    addLog("sys", "System keychain host records cleared");
  } catch (error) {
    addLog("err", `Clear failed: ${String(error)}`);
  }
}

async function loadHostsOnStartup() {
  try {
    const secureHosts = await loadHostsSecure();
    if (secureHosts.length > 0) {
      hosts.value = secureHosts;
      return;
    }
  } catch (error) {
    addLog("err", `Read keychain failed: ${String(error)}`);
  }

  const legacy = localStorage.getItem(HOST_LEGACY_STORAGE_KEY);
  if (!legacy) {
    return;
  }

  try {
    const migratedHosts = JSON.parse(legacy) as SshProfile[];
    hosts.value = migratedHosts;
    await saveHostsSecure(migratedHosts);
    localStorage.removeItem(HOST_LEGACY_STORAGE_KEY);
    addLog("sys", "Migrated legacy hosts into system keychain");
  } catch (error) {
    addLog("err", `Legacy migration failed: ${String(error)}`);
  }
}

function selectHost(index: number) {
  if (selectedHostIndex.value !== index) {
    disconnectTerminal(true);
  }

  selectedHostIndex.value = index;
  const path = activeHost.value?.basePath?.trim() || ".";
  currentPath.value = path;
  addLog("sys", `Host selected: ${activeHost.value?.name}`);

  if (activeTab.value === "files") {
    refreshDir(path);
  }
}

function saveHost() {
  const draft = { ...hostDraft.value };
  if (!draft.name || !draft.host || !draft.username || !draft.port) {
    addLog("warn", "Host info is incomplete");
    return;
  }

  if (draft.authType === "password" && !draft.password) {
    addLog("warn", "Password is required for password auth");
    return;
  }

  if (draft.authType === "key" && !draft.keyPath) {
    addLog("warn", "Key path is required for key auth");
    return;
  }

  const tags = hostTagsInput.value
    .split(",")
    .map((item) => item.trim())
    .filter(Boolean);

  draft.group = draft.group?.trim() || "Default";
  draft.tags = tags;
  hosts.value.push(draft);

  hostDraft.value = createEmptyHost();
  hostTagsInput.value = "";
  showHostForm.value = false;
}

function removeHost(index: number) {
  hosts.value.splice(index, 1);
  if (selectedHostIndex.value === index) {
    disconnectTerminal(true);
    selectedHostIndex.value = -1;
  }
}

async function pingHost(index: number) {
  const host = hosts.value[index];
  if (!host) {
    return;
  }

  try {
    const message = await testConnection(host);
    addLog("sys", message);
  } catch (error) {
    addLog("err", String(error));
  }
}

function startPtyPolling() {
  stopPtyPolling();
  ptyPollTimer = window.setInterval(async () => {
    if (!ptySessionId.value || pollingPty.value) {
      return;
    }

    pollingPty.value = true;
    try {
      const response = await readPtyOutput(ptySessionId.value);
      if (response.output) {
        addLog("out", response.output);
      }
    } catch (error) {
      addLog("err", `Read PTY output failed: ${String(error)}`);
      await disconnectTerminal(true);
    } finally {
      pollingPty.value = false;
    }
  }, 220);
}

function stopPtyPolling() {
  if (ptyPollTimer) {
    window.clearInterval(ptyPollTimer);
    ptyPollTimer = null;
  }
}

async function connectTerminal() {
  if (!activeHost.value || ptyConnected.value || ptyConnecting.value) {
    return;
  }

  ptyConnecting.value = true;
  try {
    await testConnection(activeHost.value);
    ptySessionId.value = await openPtySession(activeHost.value);
    ptyConnected.value = true;
    startPtyPolling();
    addLog("sys", "PTY session created and long-lived connection is active");
  } catch (error) {
    addLog("err", `Connect PTY failed: ${String(error)}`);
  } finally {
    ptyConnecting.value = false;
  }
}

async function disconnectTerminal(notify = true) {
  stopPtyPolling();

  if (ptySessionId.value) {
    try {
      await closePtySession(ptySessionId.value);
    } catch (error) {
      if (notify) {
        addLog("err", `Close PTY failed: ${String(error)}`);
      }
    }
  }

  ptySessionId.value = "";
  ptyConnected.value = false;

  if (notify) {
    addLog("sys", "PTY session disconnected");
  }
}

async function ensureTerminalConnected() {
  if (ptyConnected.value) {
    return true;
  }

  await connectTerminal();
  return ptyConnected.value;
}

async function executeCommand() {
  if (!activeHost.value || !commandInput.value.trim()) {
    return;
  }

  const cmd = commandInput.value.trim();

  try {
    const audit = await auditShellCommand(cmd);
    lastAudit.value = audit;

    if (audit.blocked) {
      addLog("warn", `Command blocked: ${audit.reason}`);
      return;
    }

    if (audit.requiresConfirmation) {
      const ok = window.confirm(`Risky command confirmation\n\n${audit.reason}\n\nCommand: ${cmd}`);
      if (!ok) {
        addLog("sys", "Command execution cancelled");
        return;
      }
    }

    const connected = await ensureTerminalConnected();
    if (!connected || !ptySessionId.value) {
      addLog("err", "PTY is offline, command cannot run");
      return;
    }

    addLog("cmd", `$ ${cmd}`);
    await sendPtyInput(ptySessionId.value, `${cmd}\n`);
    commandInput.value = "";
  } catch (error) {
    addLog("err", `Command execution failed: ${String(error)}`);
  }
}

async function sendCtrlC() {
  if (!ptySessionId.value) {
    return;
  }

  try {
    await sendPtyInput(ptySessionId.value, "\u0003");
    addLog("sys", "Ctrl+C sent");
  } catch (error) {
    addLog("err", `Send Ctrl+C failed: ${String(error)}`);
  }
}

function clearLogs() {
  logs.value = [];
}

function onCommandDrop(event: DragEvent) {
  event.preventDefault();
  const text = event.dataTransfer?.getData("text/plain")?.trim();
  if (!text) {
    return;
  }
  commandInput.value = commandInput.value ? `${commandInput.value} ${text}` : text;
  activeTab.value = "terminal";
}
function joinRemotePath(base: string, name: string) {
  const trimmedBase = base.trim();
  if (!trimmedBase || trimmedBase === ".") {
    return name;
  }
  if (trimmedBase.endsWith("/")) {
    return `${trimmedBase}${name}`;
  }
  return `${trimmedBase}/${name}`;
}

async function refreshDir(path = currentPath.value) {
  if (!activeHost.value) {
    return;
  }

  fileBusy.value = true;
  try {
    const targetPath = path.trim() || ".";
    const entries = await listRemoteDir(activeHost.value, targetPath);
    fileEntries.value = entries;
    currentPath.value = targetPath;
  } catch (error) {
    addLog("err", String(error));
  } finally {
    fileBusy.value = false;
  }
}

function goUp() {
  if (!currentPath.value || currentPath.value === "/" || currentPath.value === ".") {
    return;
  }

  const parts = currentPath.value.split("/").filter(Boolean);
  parts.pop();
  const parent = `/${parts.join("/")}` || "/";
  refreshDir(parent);
}

async function openEntry(path: string, isDir: boolean) {
  closeFileContextMenu();

  if (isDir) {
    await refreshDir(path);
    return;
  }

  if (!activeHost.value) {
    return;
  }

  fileBusy.value = true;
  try {
    selectedFileContent.value = await readRemoteFile(activeHost.value, path);
    selectedFilePath.value = path;
  } catch (error) {
    addLog("err", String(error));
  } finally {
    fileBusy.value = false;
  }
}

async function saveCurrentFile() {
  if (!activeHost.value || !selectedFilePath.value) {
    return;
  }

  fileBusy.value = true;
  try {
    const message = await writeRemoteFile(activeHost.value, selectedFilePath.value, selectedFileContent.value);
    addLog("sys", message);
  } catch (error) {
    addLog("err", String(error));
  } finally {
    fileBusy.value = false;
  }
}

async function downloadEntry(path: string) {
  if (!activeHost.value) {
    return;
  }

  try {
    const payload = await downloadRemoteFile(activeHost.value, path);
    const bytes = base64ToBytes(payload.dataBase64);
    const blob = new Blob([bytes]);
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = payload.name;
    a.click();
    URL.revokeObjectURL(url);
    addLog("sys", `Download success: ${payload.name}`);
  } catch (error) {
    addLog("err", `Download failed: ${String(error)}`);
  }
}

async function downloadCurrentFile() {
  if (!selectedFilePath.value) {
    return;
  }
  await downloadEntry(selectedFilePath.value);
}

function triggerUpload() {
  uploadInputRef.value?.click();
}

async function onUploadInputChange(event: Event) {
  const input = event.target as HTMLInputElement;
  const files = input.files;
  if (!files || files.length === 0) {
    return;
  }

  await uploadFiles(files);
  input.value = "";
}

async function onFileDrop(event: DragEvent) {
  const files = event.dataTransfer?.files;
  if (!files || files.length === 0) {
    return;
  }

  await uploadFiles(files);
}

async function uploadFiles(files: FileList) {
  if (!activeHost.value) {
    return;
  }

  fileBusy.value = true;
  try {
    for (const file of Array.from(files)) {
      const contentBase64 = await fileToBase64(file);
      const remotePath = joinRemotePath(currentPath.value, file.name);
      await uploadRemoteFile(activeHost.value, remotePath, contentBase64);
      addLog("sys", `Upload success: ${remotePath}`);
    }
    await refreshDir(currentPath.value);
  } catch (error) {
    addLog("err", `Upload failed: ${String(error)}`);
  } finally {
    fileBusy.value = false;
  }
}

function openFileContextMenu(event: MouseEvent, entry: RemoteEntry) {
  fileMenu.value = {
    visible: true,
    x: event.clientX,
    y: event.clientY,
    entry
  };
}

function closeFileContextMenu() {
  fileMenu.value.visible = false;
  fileMenu.value.entry = null;
}

async function contextOpen() {
  const entry = fileMenu.value.entry;
  if (!entry) {
    return;
  }
  await openEntry(entry.path, entry.is_dir);
}

async function contextDownload() {
  const entry = fileMenu.value.entry;
  closeFileContextMenu();
  if (!entry || entry.is_dir) {
    return;
  }
  await downloadEntry(entry.path);
}

async function contextRename() {
  const entry = fileMenu.value.entry;
  closeFileContextMenu();
  if (!entry || !activeHost.value) {
    return;
  }

  const nextName = window.prompt("Input new name", entry.name);
  if (!nextName || nextName === entry.name) {
    return;
  }

  const parent = entry.path.includes("/") ? entry.path.substring(0, entry.path.lastIndexOf("/")) : ".";
  const newPath = joinRemotePath(parent || ".", nextName);

  try {
    const message = await renameRemotePath(activeHost.value, entry.path, newPath);
    addLog("sys", message);
    await refreshDir(currentPath.value);
  } catch (error) {
    addLog("err", `Rename failed: ${String(error)}`);
  }
}

async function contextDelete() {
  const entry = fileMenu.value.entry;
  closeFileContextMenu();
  if (!entry || !activeHost.value) {
    return;
  }

  const ok = window.confirm(`Delete ${entry.path} ?`);
  if (!ok) {
    return;
  }

  try {
    const message = await deleteRemotePath(activeHost.value, entry.path, entry.is_dir);
    addLog("sys", message);
    await refreshDir(currentPath.value);
  } catch (error) {
    addLog("err", `Delete failed: ${String(error)}`);
  }
}

async function promptCreateFolder() {
  if (!activeHost.value) {
    return;
  }

  const name = window.prompt("Input folder name");
  if (!name) {
    return;
  }

  const remotePath = joinRemotePath(currentPath.value, name);
  try {
    const message = await createRemoteDir(activeHost.value, remotePath);
    addLog("sys", message);
    await refreshDir(currentPath.value);
  } catch (error) {
    addLog("err", `Create folder failed: ${String(error)}`);
  }
}

function fileToBase64(file: File) {
  return new Promise<string>((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => {
      const value = String(reader.result || "");
      const comma = value.indexOf(",");
      resolve(comma >= 0 ? value.slice(comma + 1) : value);
    };
    reader.onerror = () => reject(reader.error || new Error("Read file failed"));
    reader.readAsDataURL(file);
  });
}

function base64ToBytes(base64: string) {
  const binary = window.atob(base64);
  const bytes = new Uint8Array(binary.length);
  for (let index = 0; index < binary.length; index += 1) {
    bytes[index] = binary.charCodeAt(index);
  }
  return bytes;
}

function onAiDragStart(event: DragEvent, content: string) {
  event.dataTransfer?.setData("text/plain", content);
}

function appendToCommand(content: string) {
  const clean = content.trim();
  if (!clean) {
    return;
  }
  commandInput.value = commandInput.value ? `${commandInput.value} ${clean}` : clean;
  activeTab.value = "terminal";
}

function applyPresetById(presetId: string) {
  const preset = MODEL_PRESETS.find((item) => item.id === presetId);
  if (!preset) {
    return;
  }

  aiConfig.value = {
    ...aiConfig.value,
    ...preset.config
  };
  systemPrompt.value = preset.systemPrompt;
}

function applyPromptTemplate(templateId: string) {
  const template = PROMPT_TEMPLATES.find((item) => item.id === templateId);
  if (!template) {
    return;
  }

  aiPrompt.value = template.prompt;
}

async function sendAiMessage() {
  const prompt = aiPrompt.value.trim();
  if (!prompt || aiBusy.value) {
    return;
  }

  const conversation: AiMessage[] = [
    {
      role: "system",
      content: systemPrompt.value.trim() || DEFAULT_SYSTEM_PROMPT
    },
    ...aiMessages.value,
    {
      role: "user",
      content: prompt
    }
  ];

  aiMessages.value.push({ role: "user", content: prompt });
  aiPrompt.value = "";
  aiBusy.value = true;

  try {
    const response = await chatWithAi(aiConfig.value, conversation);
    aiMessages.value.push({ role: "assistant", content: response.content });
  } catch (error) {
    aiMessages.value.push({
      role: "assistant",
      content: `AI request failed: ${String(error)}`
    });
  } finally {
    aiBusy.value = false;
  }
}
function onGlobalPointerDown() {
  closeFileContextMenu();
}

function onGlobalKeyDown(event: KeyboardEvent) {
  if (event.key === "Escape") {
    closeFileContextMenu();
  }
}

async function minimizeWindow() {
  try {
    await getCurrentWindow().minimize();
  } catch {
    // browser mode
  }
}

async function toggleMaximize() {
  try {
    await getCurrentWindow().toggleMaximize();
  } catch {
    // browser mode
  }
}

async function closeWindow() {
  try {
    await getCurrentWindow().close();
  } catch {
    // browser mode
  }
}

onMounted(async () => {
  document.addEventListener("pointerdown", onGlobalPointerDown);
  document.addEventListener("keydown", onGlobalKeyDown);

  await loadHostsOnStartup();
  hostStorageReady.value = true;

  if (hosts.value.length > 0) {
    selectHost(0);
  }

  const aiRaw = localStorage.getItem(AI_STORAGE_KEY);
  if (aiRaw) {
    try {
      aiConfig.value = {
        ...aiConfig.value,
        ...JSON.parse(aiRaw)
      };
    } catch {
      // ignore
    }
  }

  const promptRaw = localStorage.getItem(SYSTEM_PROMPT_STORAGE_KEY);
  if (promptRaw) {
    systemPrompt.value = promptRaw;
  }

  const presetRaw = localStorage.getItem(PRESET_STORAGE_KEY);
  if (presetRaw) {
    selectedPresetId.value = presetRaw;
    applyPresetById(presetRaw);
  } else {
    applyPresetById(selectedPresetId.value);
  }
});

onBeforeUnmount(() => {
  document.removeEventListener("pointerdown", onGlobalPointerDown);
  document.removeEventListener("keydown", onGlobalKeyDown);
  stopPtyPolling();
  disconnectTerminal(true);
});
</script>
