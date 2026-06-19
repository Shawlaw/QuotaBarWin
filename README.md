# QuotaBarWin

默认语言：简体中文。English documentation:
[`README.en.md`](README.en.md).

QuotaBarWin 是一个 Windows-first 的 AI 用量 / 额度监控工具，基于 Tauri 2、
Rust、React 和 Vite 构建。

核心架构原则：**Provider 是唯一对外数据抽象**。用户可配置的 Provider 统一来自
remote registry / manifest；Provider 脚本内部可以使用 CLI、API 请求、JSON 解析或
文本解析，但前端只渲染归一化后的 `ProviderSnapshot` / `AppSnapshot` 数据。

## 当前功能

- 桌面概览页：Provider 卡片、额度窗口、进度条、全局状态和单 Provider 刷新。
- Windows 托盘、托盘弹窗、隐藏启动和单实例行为。
- 可配置刷新间隔、显示模式、低额度警告阈值、语言、日志级别、开机启动和
  Provider 顺序。
- 支持 AppData 配置和便携模式。将 `quotabarwin.portable` 放在可执行文件旁
  即启用便携模式。
- 支持 `visibleWindowIds` 和 `windowLabelOverrides` 自定义额度窗口显示。
- 远程 Provider registry：从 manifest 安装缓存脚本，支持可选 SHA-256 校验
  和更新检查；设置页显示 Provider 版本、安装时间、更新时间和上次检查时间。
- 全局代理和单 Provider 代理，支持 HTTP 与 SOCKS5。
- Provider 配置支持 secret 占位符：`${secret:NAME}`、`${env:NAME}`、
  `${file:C:\path\secret.txt}`。
- 支持导出已脱敏的诊断 zip。

## Provider 模型

当前实现只支持一种用户配置 Provider：

| Kind | 来源 | 说明 |
|---|---|---|
| `remote` | 缓存外部脚本 | 从 registry / manifest 安装，并使用声明的 runtime 执行，例如 `node`、`python`、`pwsh`、`bash` 或绝对路径。 |

旧版 `mock` / `codex` / `command` / `script` Provider 仍可在历史 specs 中看到，
但当前 config schema 不再接受它们。Codex usage 通过 `codex-usage` remote
Provider 示例提供。

远程 Provider 作者指南：
[`docs/remote-provider-guide.md`](docs/remote-provider-guide.md)。完整示例位于
[`examples/remote-providers/`](examples/remote-providers/)。

## 配置存储

当前配置 schema version：`11`。

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

`${secret:NAME}` 会优先读取 `<config-dir>\secrets\NAME.txt`，找不到时回退到进程
环境变量 `NAME`。

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

## 构建产物

Windows release 由 `.github/workflows/release.yml` 生成。工作流会构建 Tauri
安装包，并额外生成包含 `QuotaBarWin.exe` 的 portable zip。

本地安装包构建：

```powershell
npm run tauri build
```

Tauri MSI / NSIS 打包配置位于
[`src-tauri/tauri.conf.json`](src-tauri/tauri.conf.json)。

## 安装

从 release artifacts 下载 Windows installer 并运行。也可以解压 portable zip 到任意
目录后直接启动 `QuotaBarWin.exe`。

## 卸载

如果使用 Windows installer 安装，请在 Windows 设置 > 应用 > 已安装应用 >
QuotaBarWin 中卸载。如果使用 portable zip，请先从托盘菜单退出 QuotaBarWin，然后
删除解压目录。

如需同时删除本地设置、诊断、日志、secrets 和远程 Provider 缓存，请删除
`%APPDATA%\QuotaBarWin`。
