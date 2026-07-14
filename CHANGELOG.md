# 变更记录

本文记录 QuotaBarWin 的用户可见变化。默认以简体中文维护；如果未来需要英文 release notes，可从本文件同步整理。

## [1.0.3] - 2026-07-15

### Changed

- 安装源代理仅用于下载 registry、manifest 和 Provider 脚本；Provider 运行期代理改为优先使用其环境变量 `QBWIN_PROXY_URL`，未设置时才使用项目全局代理兜底。
- 配置 schema 升至 16；升级时会清除旧版遗留的 Provider `proxyUrl`，避免它覆盖 Provider 自己的运行期代理配置。

### Fixed

- 修复 Provider 目录加载时旧网络请求晚于新请求返回，导致成功列表被“安装源没有数据”错误覆盖的问题。

## [1.0.2] - 2026-07-14

### Added

- 新增“光阴似箭”Provider：按分钟显示本日、本周（周一/周日开始）、本月和本年的剩余时间。
- 设置页可单独控制每个 Provider 是否显示在托盘小窗，默认显示。

### Changed

- 主窗口和托盘小窗将额度的详细数值改为悬停显示，界面默认更简洁；重置时间仍保持可见。

### Fixed

- 修复 portable 模式仍把远程 Provider 缓存写入 AppData 的问题。缓存现在随当前配置目录存放，切换模式和首次启动新版时会迁移旧缓存；迁移失败会回退，避免半迁移状态。

## [1.0.1] - 2026-07-14

### Fixed

- 保存远程 Provider 来源后保留设置页、返回“添加提供方”并立即使用最新代理刷新目录。
- 来源代理输入提示明确支持 HTTP 与 SOCKS5 URL。
- 修复单个额度窗口时托盘弹窗自动高度将空白区域计入内容高度的问题，并避免用户调整窗口时触发自动高度循环。

## [1.0.0] - 初始发布

### Added

- Windows-first Tauri 2 桌面应用，提供主窗口和托盘弹窗。
- Remote Provider 模型，支持从 registry / manifest 安装外部 Provider 脚本。
- Provider 启用、排序、窗口显示控制、窗口名称覆盖、手动刷新和自动刷新。
- AppData 与 portable 配置模式，包含日志、secrets、远程 Provider 缓存和 snapshot 磁盘缓存。
- 全局和单 Provider 网络代理，支持 HTTP 与 SOCKS5。
- secret 占位符：`${secret:NAME}`、`${env:NAME}`、`${file:C:\path\secret.txt}`。
- 诊断导出、结构化脱敏日志和日志轮转。
- Windows portable zip release workflow。

### Security

- 远程 Provider stdout 只接受最终 JSON payload，stderr 进入脱敏日志和诊断。
- snapshot 磁盘缓存会剥离 Provider `metadata`。
- Tauri WebView 启用基础 CSP。
- 开源前补充安全披露流程、依赖审查、Dependabot 和 npm audit 检查。
