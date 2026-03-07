# Hiphup SSH

> 一个类似 Smartty 的跨平台远程 Shell 桌面工具：`Vue3 + Rust + Tauri`。
> 支持 SSH 命令、远程文件图形管理、AI 终端助手（可接 OpenAI/Anthropic/Ollama/兼容接口），并支持回答拖拽到命令输入框。

## 项目亮点

- 左侧主机管理：保存多个 SSH 连接（密码 / 私钥）。
- 中间终端工作区：执行远程命令并展示 stdout/stderr/退出码。
- 文件图形区：远程目录浏览、文件读取、编辑、保存。
- 右侧 AI 助手：
  - 支持多模型提供商接入（OpenAI、Anthropic、Ollama、OpenAI 兼容网关）。
  - AI 回复可直接拖拽到 Shell 命令输入框。
  - 也支持一键插入命令框。
- 窗口操作：自定义顶部栏，支持最小化/最大化/关闭。
- 跨平台打包：`exe/nsis/msi`（Windows）+ `app/dmg`（macOS）+ `apk`（Android）。

## 技术栈

- 前端：Vue 3 + TypeScript + Vite
- 桌面容器：Tauri 2
- 后端：Rust
- SSH：`ssh2`
- AI HTTP：`reqwest`

## 目录结构

```text
smart-shell/
  src/                       # Vue3 前端
  src-tauri/
    src/main.rs              # Rust 命令：SSH + 文件 + AI
    tauri.conf.json          # Tauri 应用与打包配置
    capabilities/default.json
  .github/workflows/release.yml
  README.md
```

## 快速开始

### 1. 安装依赖

```bash
npm install
```

### 2. 本地开发

```bash
npm run tauri:dev
```

### 3. 生产打包

```bash
npm run tauri:build
```

构建产物在 `src-tauri/target/release/bundle/` 下。

## AI 接入说明

右侧 AI 面板中配置：

- `provider`：`openai` / `anthropic` / `ollama` / `openai_compatible`
- `endpoint`：模型接口地址
- `model`：模型名
- `apiKey`：密钥（Ollama 本地通常可留空）

### 示例 Endpoint

- OpenAI: `https://api.openai.com/v1/chat/completions`
- Anthropic: `https://api.anthropic.com/v1/messages`
- Ollama: `http://127.0.0.1:11434/api/chat`

## 发布到 GitHub

### 1. 初始化仓库并提交

```bash
git init
git add .
git commit -m "feat: init hiphup ssh desktop app"
```

### 2. 关联远端

```bash
git branch -M main
git remote add origin <你的仓库地址>
git push -u origin main
```

### 3. 触发自动发布

推送标签（例如 `v1.0.0`）会触发 `.github/workflows/release.yml`：

```bash
git tag v1.0.0
git push origin v1.0.0
```

GitHub Actions 会自动构建并发布安装包。

## 后续建议（可选）

- 接入真实 PTY 交互（保持长连接，而不只是单次命令执行）。
- 增加 SFTP 上传/下载、拖拽上传、右键菜单。
- 支持模型预设模板和命令安全审计。
- 增加主机分组、标签、加密存储（如系统密钥链）。

## 说明

- 当前版本已按你提供的界面方向实现核心交互与布局。
- README 结构采用 RT / Collector 常见风格：功能、架构、启动、发布、后续规划。


