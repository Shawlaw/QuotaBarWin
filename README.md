<p align="center">
  <img src="src-tauri/icons/source.png" width="128" alt="QuotaBarWin 图标">
</p>

<h1 align="center">QuotaBarWin</h1>

QuotaBarWin 是一个 Windows-first 的 AI 用量 / 额度监控桌面工具，用来把多个 AI 服务的额度窗口统一展示在主窗口和托盘弹窗里。

默认语言：简体中文。English documentation: [README.en.md](README.en.md).

---

## 文档入口

- [English README](README.en.md)
- [远程 Provider 作者指南](docs/remote-provider-guide.md)
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

## 核心能力

- 在概览页按 Provider 展示额度窗口、剩余额度、重置时间、状态和进度条。
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

不要把真实 API key、token、cookie、账号 ID 或代理凭据写入代码、文档、fixtures 或测试。

---

## 首次使用

1. 从 GitHub Release 下载 Windows portable zip。
2. 解压到任意目录，运行 `QuotaBarWin.exe`。
3. 进入设置页，按需调整刷新间隔、语言、代理、开机启动和日志级别。
4. 在 Provider 区域添加或更新远程 Provider。
5. 回到概览页或托盘弹窗查看额度状态。

---

## 开发

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

注意：E2E 当前是维护者可选检查，运行前请先查看 `e2e/tauri.e2e.mjs`。它会构建 release app，并写入临时 portable 配置；历史上该脚本可能落后于 Provider schema，不能替代上面的常规构建和单元测试。

---

## 构建和发布

本地 release exe 构建：

```powershell
npm run tauri -- build --no-bundle
```

Windows release 由 `.github/workflows/release.yml` 生成。发布工作流构建 `QuotaBarWin.exe`，并打包为 portable zip，文件名格式为：

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
