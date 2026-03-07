<template>
  <div class="app-shell">
    <header class="window-bar">
      <div class="window-dots">
        <button class="dot close" @click="closeWindow" aria-label="close"></button>
        <button class="dot min" @click="minimizeWindow" aria-label="minimize"></button>
        <button class="dot max" @click="toggleMaximize" aria-label="maximize"></button>
      </div>
      <div class="brand">HIPHUP SSH V2.0</div>
    </header>

    <main class="layout">
      <aside class="hosts-panel">
        <div class="hosts-header">
          <div class="logo">H</div>
          <div>
            <h1>Hiphup</h1>
            <p>HOSTS</p>
          </div>
        </div>

        <div class="hosts-list">
          <button
            v-for="(host, index) in hosts"
            :key="`${host.host}-${index}`"
            :class="['host-card', { active: index === selectedHostIndex }]"
            @click="selectHost(index)"
          >
            <div>
              <strong>{{ host.name }}</strong>
              <small>{{ host.username }}@{{ host.host }}:{{ host.port }}</small>
            </div>
            <div class="host-actions">
              <button @click.stop="pingHost(index)">测</button>
              <button @click.stop="removeHost(index)">删</button>
            </div>
          </button>
        </div>

        <button class="add-host" @click="showHostForm = !showHostForm">
          + Add New Host
        </button>

        <form v-if="showHostForm" class="host-form" @submit.prevent="saveHost">
          <input v-model="hostDraft.name" placeholder="名称" required />
          <input v-model="hostDraft.host" placeholder="IP/域名" required />
          <input v-model.number="hostDraft.port" type="number" placeholder="端口" required />
          <input v-model="hostDraft.username" placeholder="用户名" required />
          <select v-model="hostDraft.authType">
            <option value="password">密码</option>
            <option value="key">私钥</option>
          </select>
          <input
            v-if="hostDraft.authType === 'password'"
            v-model="hostDraft.password"
            type="password"
            placeholder="密码"
            required
          />
          <input
            v-else
            v-model="hostDraft.keyPath"
            placeholder="私钥路径，如 C:/Users/me/.ssh/id_rsa"
            required
          />
          <input v-model="hostDraft.basePath" placeholder="默认目录，如 /home/ubuntu" />
          <button type="submit">保存主机</button>
        </form>
      </aside>

      <section class="workspace">
        <div class="workspace-header">
          <span>Hosts / <strong>{{ activeHost?.name || 'Select a host' }}</strong></span>
          <div class="tabs">
            <button :class="{ active: activeTab === 'terminal' }" @click="activeTab = 'terminal'">终端</button>
            <button :class="{ active: activeTab === 'files' }" @click="activeTab = 'files'">文件</button>
          </div>
        </div>

        <div v-if="activeTab === 'terminal'" class="terminal-panel">
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
              placeholder="输入 Linux 命令，或把 AI 回复拖拽到这里"
              :disabled="!activeHost || commandBusy"
              @keydown.enter.prevent="executeCommand"
              @dragover.prevent
              @drop="onCommandDrop"
            />
            <button :disabled="!activeHost || commandBusy || !commandInput.trim()" @click="executeCommand">
              {{ commandBusy ? '执行中...' : '执行' }}
            </button>
          </div>
        </div>

        <div v-else class="files-panel">
          <div class="file-toolbar">
            <button :disabled="!activeHost || fileBusy" @click="goUp">上级</button>
            <button :disabled="!activeHost || fileBusy" @click="refreshDir()">刷新</button>
            <input v-model="currentPath" :disabled="!activeHost || fileBusy" @keydown.enter.prevent="refreshDir(currentPath)" />
          </div>

          <div class="file-content">
            <div class="tree">
              <button
                v-for="entry in sortedEntries"
                :key="entry.path"
                class="entry"
                @dblclick="openEntry(entry.path, entry.is_dir)"
              >
                <span>{{ entry.is_dir ? '📁' : '📄' }}</span>
                <span>{{ entry.name }}</span>
              </button>
            </div>

            <div class="editor">
              <div class="editor-header">
                <span>{{ selectedFilePath || '双击文件进行编辑' }}</span>
                <button :disabled="!selectedFilePath || fileBusy" @click="saveCurrentFile">保存</button>
              </div>
              <textarea
                v-model="selectedFileContent"
                :disabled="!selectedFilePath || fileBusy"
                placeholder="文件内容"
              ></textarea>
            </div>
          </div>
        </div>
      </section>

      <aside class="ai-panel">
        <div class="ai-header">
          <h2>AI 终端助手</h2>
          <button @click="showAiConfig = !showAiConfig">⚙</button>
        </div>

        <div v-if="showAiConfig" class="ai-config">
          <select v-model="aiConfig.provider">
            <option value="openai">OpenAI</option>
            <option value="anthropic">Anthropic</option>
            <option value="ollama">Ollama</option>
            <option value="openai_compatible">OpenAI Compatible</option>
          </select>
          <input v-model="aiConfig.endpoint" placeholder="Endpoint" />
          <input v-model="aiConfig.model" placeholder="Model" />
          <input v-model="aiConfig.apiKey" type="password" placeholder="API Key (Ollama 可留空)" />
          <label>
            Temperature
            <input v-model.number="aiConfig.temperature" type="number" min="0" max="2" step="0.1" />
          </label>
        </div>

        <div class="messages">
          <article
            v-for="(message, index) in aiMessages"
            :key="`${message.role}-${index}`"
            :class="['bubble', message.role]"
          >
            <header>{{ message.role === 'assistant' ? 'AI' : '你' }}</header>
            <p class="bubble-text">{{ message.content }}</p>
            <div v-if="message.role === 'assistant'" class="bubble-actions">
              <button @click="appendToCommand(message.content)">插入命令框</button>
              <button draggable="true" @dragstart="onAiDragStart($event, message.content)">拖拽整段</button>
            </div>
          </article>
        </div>

        <div class="chat-input-row">
          <textarea
            v-model="aiPrompt"
            placeholder="询问 Linux 相关问题..."
            @keydown.ctrl.enter.prevent="sendAiMessage"
          ></textarea>
          <button :disabled="aiBusy || !aiPrompt.trim()" @click="sendAiMessage">
            {{ aiBusy ? '思考中...' : '发送' }}
          </button>
        </div>
      </aside>
    </main>
  </div>
</template>

<script setup lang="ts">
import { computed, nextTick, onMounted, ref, watch } from "vue";
import { getCurrentWindow } from "@tauri-apps/api/window";
import {
  chatWithAi,
  listRemoteDir,
  readRemoteFile,
  runRemoteCommand,
  testConnection,
  writeRemoteFile
} from "./composables/useBackend";
import type { AiConfig, AiMessage, RemoteEntry, SshProfile } from "./types";

type LogKind = "cmd" | "out" | "err" | "sys";

interface LogItem {
  kind: LogKind;
  text: string;
  ts: number;
}

const HOST_STORAGE_KEY = "hiphup-hosts-v1";
const AI_STORAGE_KEY = "hiphup-ai-config-v1";

const hosts = ref<SshProfile[]>([]);
const selectedHostIndex = ref<number>(-1);
const showHostForm = ref(false);
const hostDraft = ref<SshProfile>(createEmptyHost());

const activeTab = ref<"terminal" | "files">("terminal");
const logs = ref<LogItem[]>([]);
const commandInput = ref("");
const commandBusy = ref(false);
const logRef = ref<HTMLElement | null>(null);

const fileBusy = ref(false);
const currentPath = ref(".");
const fileEntries = ref<RemoteEntry[]>([]);
const selectedFilePath = ref("");
const selectedFileContent = ref("");

const aiBusy = ref(false);
const showAiConfig = ref(false);
const aiPrompt = ref("");
const aiMessages = ref<AiMessage[]>([
  {
    role: "assistant",
    content: "你好！我是你的 SSH 助手。配置右上角 API 后即可接入大模型，支持将回复拖拽到命令输入框。"
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

watch(
  hosts,
  () => {
    localStorage.setItem(HOST_STORAGE_KEY, JSON.stringify(hosts.value));
  },
  { deep: true }
);

function createEmptyHost(): SshProfile {
  return {
    name: "",
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

function selectHost(index: number) {
  selectedHostIndex.value = index;
  const path = activeHost.value?.basePath?.trim() || ".";
  currentPath.value = path;
  addLog("sys", `已选中主机: ${activeHost.value?.name}`);
  if (activeTab.value === "files") {
    refreshDir(path);
  }
}

function saveHost() {
  const draft = { ...hostDraft.value };
  if (!draft.name || !draft.host || !draft.username || !draft.port) {
    return;
  }

  if (draft.authType === "password" && !draft.password) {
    return;
  }

  if (draft.authType === "key" && !draft.keyPath) {
    return;
  }

  hosts.value.push(draft);
  hostDraft.value = createEmptyHost();
  showHostForm.value = false;
}

function removeHost(index: number) {
  hosts.value.splice(index, 1);
  if (selectedHostIndex.value === index) {
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

async function executeCommand() {
  if (!activeHost.value || !commandInput.value.trim()) {
    return;
  }

  const cmd = commandInput.value.trim();
  commandBusy.value = true;
  addLog("cmd", `$ ${cmd}`);

  try {
    const result = await runRemoteCommand(activeHost.value, cmd);
    if (result.stdout) {
      addLog("out", result.stdout);
    }
    if (result.stderr) {
      addLog("err", result.stderr);
    }
    addLog("sys", `退出码: ${result.exit_code}`);
  } catch (error) {
    addLog("err", String(error));
  } finally {
    commandBusy.value = false;
    commandInput.value = "";
  }
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

async function sendAiMessage() {
  const prompt = aiPrompt.value.trim();
  if (!prompt || aiBusy.value) {
    return;
  }

  const conversation: AiMessage[] = [
    {
      role: "system",
      content:
        "你是 Linux/SSH 自动化助手。回答优先给可执行命令、简短解释和风险提示。命令使用代码块，避免危险操作。"
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
      content: `调用 AI 失败: ${String(error)}`
    });
  } finally {
    aiBusy.value = false;
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

onMounted(() => {
  const hostRaw = localStorage.getItem(HOST_STORAGE_KEY);
  if (hostRaw) {
    try {
      hosts.value = JSON.parse(hostRaw);
      if (hosts.value.length > 0) {
        selectHost(0);
      }
    } catch {
      hosts.value = [];
    }
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
});
</script>
