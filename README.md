<p align="center">
  <img src="src-tauri/icons/source.png" width="128" alt="QuotaBarWin 图标">
</p>

<h1 align="center">QuotaBarWin</h1>

QuotaBarWin 是一个仅支持 Windows 的 AI 用量 / 额度监控桌面工具，用来把多个 AI 服务的额度窗口统一展示在主窗口和托盘弹窗里。

macOS 用户可以使用或参考 [CodexBar](https://github.com/steipete/CodexBar)，它是一个 macOS 菜单栏 AI 用量监控工具。

默认语言：简体中文。English documentation: [README.en.md](README.en.md).

---

## 文档入口

- [English README](README.en.md)
- [远程 Provider 作者指南](docs/remote-provider-guide.md)
- [Agent / CLI 使用指南](docs/cli.md)
- [远程 Provider 示例](examples/remote-providers/)
- [远程 Provider registry 示例](examples/remote-providers/registry.json)
- [贡献指南](CONTRIBUTING.md)
- [安全政策](SECURITY.md)
- [行为准则](CODE_OF_CONDUCT.md)
- [开源准备审计](docs/open-source-checklist.md)

---

## 当前发布形态

- 平台：**Windows**
- 分发方式：**绿色版 portable zip + 单 exe**
- 当前版本：**v1.0.0**
- 技术栈：Tauri 2、Rust 2021、React 19、TypeScript、Vite
- 当前配置 schema version：**14**

---

## 运行截图

主窗口提供概览页和设置页；托盘弹窗用于快速查看关键额度。

<p align="center">
  <img src="assets/screenshots/readme-zh-overview.png" alt="QuotaBarWin 中文版概览页截图" width="49%">
  <img src="assets/screenshots/readme-zh-settings.png" alt="QuotaBarWin 中文版设置页截图" width="49%">
</p>

<p align="center">
  <img src="assets/screenshots/readme-zh-tray-popup.png" alt="QuotaBarWin 中文版托盘弹窗截图" width="42%">
</p>

---

## 快速上手

QuotaBarWin 发布包不会内置你的账号凭据，也不会默认启用某个服务账号。应用默认配置了 QuotaBarWin 项目维护的远程 Provider 来源，设置页会从这个 registry 展示可安装的 Provider：

[QuotaBarWin 项目维护的 Provider registry](https://raw.githubusercontent.com/Shawlaw/QuotaBarWin/main/examples/remote-providers/registry.json)

**安全提示：只安装使用可信任的 Provider。Provider 脚本可以直接读取你配置的各类 AI 鉴权信息，并发起网络通讯。**

### 1. 安装并启动

1. 从 GitHub Release 下载 Windows portable zip，解压到任意目录。
2. 运行 `QuotaBarWin.exe`。如果是 portable zip，配置、日志、secrets 和 Provider 缓存默认都会放在 exe 旁。
3. 示例 Provider 目前都使用 `node` 作为 runtime；安装或刷新这些 Provider 前，请先确认本机能运行：

```powershell
node --version
```

### 2. 选择项目维护的 Provider

QuotaBarWin 项目维护的 registry 当前包含以下 manifest；实际可安装列表以 registry 为准：

| Manifest ID | 显示名称 | 监控内容 | 常用凭据 / 配置 |
|---|---|---|---|
| `kimi-coding` | Kimi Coding Usage | Kimi coding quota 用量 | `KIMI_API_KEY` |
| `bigmodel-coding-plan` | BigModel Coding Plan | 智谱 / BigModel coding plan 额度 | `BIGMODEL_API_KEY` |
| `codex-usage` | Codex Usage | ChatGPT / Codex 5h 与 weekly 用量 | 默认读取本机 Codex auth，可用 `CODEX_ACCESS_TOKEN` 等覆盖；代理见第 5 节。 |
| `deepseek-balance` | DeepSeek Balance | DeepSeek 按量付费余额 | `DEEPSEEK_API_KEY`，可选参考总额、低余额阈值和币种。 |

### 3. 安装 Provider

**安装前请再次确认来源可信。Provider 脚本会在你的机器上运行，可以读取你配置给它的 token、API key、Cookie、账号 ID 等鉴权信息，并访问网络。**

1. 打开 **设置 → 提供方 → 添加提供方**。
2. 默认会加载 QuotaBarWin 项目维护的 Provider 来源；如果列表为空，可以在 **管理来源** 中填入上面的 registry URL。
3. 点击需要的 Provider 的 **安装**。安装后的 Provider 默认启用，也可以在 Provider 列表里用复选框停用。
4. 同一 Provider 需要多个账号时，再次点击已安装项的 **添加账号**。

### 4. 配置凭据

回到 **设置 → 提供方**，展开刚安装的 Provider，填写 **环境变量**。建议只写 `${secret:...}`、`${env:...}` 或 `${file:...}` 占位符，不要把 token 明文粘进配置。

| Provider | 环境变量示例 | 对应 Secret 文件 |
|---|---|---|
| Kimi | `KIMI_API_KEY=${secret:KIMI_API_KEY}` | `<config-dir>\secrets\KIMI_API_KEY.txt` |
| BigModel / 智谱 | `BIGMODEL_API_KEY=${secret:BIGMODEL_API_KEY}` | `<config-dir>\secrets\BIGMODEL_API_KEY.txt` |
| DeepSeek | `DEEPSEEK_API_KEY=${secret:DEEPSEEK_API_KEY}` | `<config-dir>\secrets\DEEPSEEK_API_KEY.txt` |
| Codex usage | 默认读取本机 Codex auth | 可选配置 `CODEX_ACCESS_TOKEN`、`CODEX_ACCOUNT_ID`、`CODEX_AUTH_FILE` 或代理变量 |

`<config-dir>` 可以在 **设置 → 通用 → 配置存储** 中查看，也可以点击 **打开配置存储文件夹**。如果 `secrets` 文件夹不存在，可以手动创建；secret 文件内容只需要写入对应 token 或 API key。

### 5. Codex 代理

Codex usage 需要代理时，可以在该 Provider 的 **环境变量** 中配置，例如：

```text
HTTPS_PROXY=http://127.0.0.1:7890
```

Codex usage 脚本的代理优先级是 `QBWIN_PROXY_URL`、`HTTPS_PROXY`、`HTTP_PROXY`、`ALL_PROXY`，支持 `socks5:`、`socks5h:`、`http:` 和 `https:`。如果安装 Provider 时已经为该 Provider 配置了代理 URL，QuotaBarWin 会以 `QBWIN_PROXY_URL` 注入，优先级最高。更完整的请求、鉴权和代理说明见 [`examples/remote-providers/codex-usage/api.md`](examples/remote-providers/codex-usage/api.md)。

### 6. 多账号

多账号时，左边保持脚本需要的变量名，右边换成不同 secret 名。例如两个 Kimi 账号可以分别写：

```text
# Kimi Personal 账号实例
KIMI_API_KEY=${secret:KIMI_PERSONAL_API_KEY}
```

```text
# Kimi Work 账号实例
KIMI_API_KEY=${secret:KIMI_WORK_API_KEY}
```

对应创建：

```text
<config-dir>\secrets\KIMI_PERSONAL_API_KEY.txt
<config-dir>\secrets\KIMI_WORK_API_KEY.txt
```

### 7. 保存并查看

点击 **保存**，回到 **概览** 页，或打开托盘弹窗查看额度状态。之后可以使用全局刷新或单个 Provider 卡片上的刷新按钮更新数据。

---

## Agent / CLI

portable 发布包同时包含 `QuotaBarWin.Cli.exe`。它复用已安装 Provider、凭据和配置，为 Agent 或脚本提供只输出 JSON 的额度查询与阈值判断；不会打开窗口、托盘或启动另一个桌面应用实例。

例如，让 Agent 在开始高消耗步骤前刷新 Codex 的 5 小时窗口：

```powershell
.\QuotaBarWin.Cli.exe check --provider codex-usage --window 5h --min-remaining-percent 20
```

退出码 `0` 表示可以继续，`10` 表示低于阈值、应延后或切换任务，`11` 表示数据无法安全判断（例如 Provider 已过期），`20` 表示刷新或配置失败。默认会实时刷新；仅想读取最近磁盘快照时，显式传入 `--cached`。

完整的命令、JSON 输出和 Agent 编排示例见 [Agent / CLI 使用指南](docs/cli.md)。`resetAt` 是 Provider 给出的下一次重置或建议重查时间，不保证届时一定恢复到满额。

---

## 核心能力

- 在概览页按 Provider 展示额度窗口、剩余额度、重置时间、状态和进度条。
- 附带面向 Agent 与脚本的 JSON CLI，支持刷新、读取快照和按剩余百分比判断是否应延后任务。
- 支持单个 Provider 手动刷新，也支持按全局间隔自动刷新。
- 支持 Windows 托盘、隐藏启动、单实例运行和可调整尺寸的托盘弹窗。
- 支持刷新间隔、显示模式、低额度警告阈值、语言、日志级别、开机启动等通用设置。
- 支持 AppData 配置和便携模式；便携模式会把配置、日志、secrets 和远程 Provider 缓存放在 exe 旁。
- 支持 Provider 启用状态、排序、自定义窗口显示和窗口名称覆盖。
- 支持从 remote registry / manifest 安装 Provider，并进行缓存、SHA-256 校验和更新检查。
- 支持全局代理和单 Provider 代理，代理类型包含 HTTP 与 SOCKS5。
- 支持 secret 占位符，避免在配置和日志中直接保存真实密钥。

---

## Provider 模型

QuotaBarWin 的核心架构原则是：**Provider 是唯一对外数据抽象**。

用户可见的 Provider 统一来自 remote registry / manifest。Provider 脚本内部可以使用 API 请求、CLI 工具、JSON 解析或文本解析，但前端只消费归一化后的 `AppSnapshot`、`ProviderSnapshot` 和 `QuotaWindow` 数据。

Provider 配置类型：

| Kind | 来源 | 说明 |
|---|---|---|
| `remote` | 缓存外部脚本 | 从 registry / manifest 安装，并使用声明的 runtime 执行，例如 `node`、`python`、`pwsh`、`bash` 或绝对路径。 |

---

## 运行环境和软件依赖

需要区分主程序、远程 Provider 和开发环境三类依赖：

| 场景 | 需要安装 | 说明 |
|---|---|---|
| 使用发布版 `QuotaBarWin.exe` / `QuotaBarWin.Cli.exe` | Windows；GUI 需要 Microsoft Edge WebView2 Runtime | portable zip 包含桌面程序和 CLI；CLI 不依赖 WebView2。两者都不要求用户安装 Node.js、npm、Rust 或 Tauri CLI。 |
| 安装 / 运行远程 Provider | manifest `runtime` 对应的软件 | QuotaBarWin 会从 `PATH` 或绝对路径解析 `runtime`，安装 / 更新时用 `--version` 或 `--help` 校验。若 Provider 声明 `"runtime": "node"`，该机器就需要可执行的 `node`；声明 `python`、`pwsh` 或 `bash` 时同理。当前仓库示例 Provider 均声明 `node`。 |
| 开发、测试、构建本仓库 | Node.js 22+、npm、Rust stable / Cargo、Windows MSVC build tools | `package.json` 要求 `node >=22`，Release workflow 也使用 Node 22。`npm run tauri ...` 和 `cargo test ...` 需要 Rust 工具链；完整 E2E 还需要 `tauri-driver`。 |

Provider 脚本还可以自行调用其他 CLI 或读取本地凭据文件；这些不属于 QuotaBarWin 的统一依赖，应由对应 Provider 文档声明。例如远程 Provider 指南里的 Bash 示例需要 `jq`。

---

## 配置和隐私

Windows AppData 配置：

```text
%APPDATA%\QuotaBarWin\config.quotaBarWin.json
```

启用便携模式时，可执行文件旁的配置：

```text
<app-exe-dir>\config.quotaBarWin.json
```

远程 Provider 脚本缓存目录：

```text
%APPDATA%\QuotaBarWin\providers\remote\<provider-id>\
```

启用便携模式后，远程 Provider 缓存会改为使用 exe 同目录下的 portable 存储。

Secret 占位符：

- `${secret:NAME}`：读取 `<config-dir>\secrets\NAME.txt`，找不到时回退到环境变量 `NAME`。
- `${env:NAME}`：读取环境变量 `NAME`。
- `${file:C:\path\secret.txt}`：读取本地文件并裁剪首尾空白。

同一个 Provider 配多个账号时，在每个账号实例的“环境变量”里把脚本需要的变量名映射到不同 secret，例如
`KIMI_API_KEY=${secret:KIMI_WORK_API_KEY}` 会读取
`<config-dir>\secrets\KIMI_WORK_API_KEY.txt`。详见
[`docs/remote-provider-guide.md`](docs/remote-provider-guide.md#本地配置与-secret)。

不要把真实 API key、token、cookie、账号 ID 或代理凭据写入代码、文档、fixtures 或测试。

---

## 开发

开始前请先确认已安装上面的开发环境依赖。

安装依赖：

```powershell
npm install
```

启动浏览器预览：

```powershell
npm run dev
```

启动 Tauri 桌面应用：

```powershell
npm run tauri dev
```

构建前端资源：

```powershell
npm run build
```

运行测试：

```powershell
npm run test -- --run
cargo test --manifest-path src-tauri/Cargo.toml
```

检查图标：

```powershell
npm run icons:check
```

E2E 使用 `tauri-driver` 和 WebDriverIO：

```powershell
npm run e2e
```

如需安装 `tauri-driver`：

```powershell
cargo install tauri-driver --locked
```

注意：E2E 当前是维护者可选检查，不能替代上面的常规构建和单元测试。运行前请先退出所有已运行的 QuotaBarWin 实例；Tauri 单实例机制会把新启动重定向到已有进程。脚本会构建 release app，并写入临时 portable 配置和本地 remote Provider cache。排查失败时可设置 `QBWIN_E2E_KEEP_ARTIFACTS=1` 保留这些临时文件。

---

## 构建和发布

本地 release exe 构建：

```powershell
npm run tauri -- build --no-bundle
```

Windows release 由 `.github/workflows/release.yml` 生成。发布工作流构建 `QuotaBarWin.exe` 和 `QuotaBarWin.Cli.exe`，并打包为 portable zip，文件名格式为：

```text
QuotaBarWin_<version>_windows_x64_portable_<commit>.zip
```

zip 内包含 `quotabarwin.portable`，解压后默认使用可执行文件旁的 portable 配置。推送 `v*` tag 时，zip 会上传为 GitHub Release asset；手动触发工作流时会保留为 Actions artifact。

---

## 安装和卸载

安装：从 GitHub Release 下载 portable zip，解压到任意目录后直接启动 `QuotaBarWin.exe`。

卸载：先从托盘菜单退出 QuotaBarWin，然后删除解压目录。

portable zip 默认把设置、日志、secrets 和远程 Provider 缓存在解压目录。如果之前使用过 AppData 模式，也可以删除 `%APPDATA%\QuotaBarWin`。

---

## 排障建议

- 如果额度长时间不更新，先在设置页检查 Provider 是否启用，再执行单 Provider 刷新或全局刷新。
- 如果远程 Provider 安装或更新失败，优先检查 registry / manifest 地址、网络代理和 SHA-256 校验值。
- 如果 Provider 需要密钥，优先使用 secret 占位符，不要把密钥明文写进配置。
- 如果需要反馈问题，请附上应用日志中的相关时间段，并注意手动删除敏感信息。

---

## 开源协作

- 许可证：[MIT](LICENSE)
- 贡献流程：[CONTRIBUTING.md](CONTRIBUTING.md)
- 安全漏洞披露：[SECURITY.md](SECURITY.md)
- 行为准则：[CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md)

提交 issue 或 PR 前，请先确认没有包含真实 API key、token、cookie、账号 ID、日志原文中的敏感片段或代理凭据。
