# 变更记录

本文记录 QuotaBarWin 的用户可见变化。默认以简体中文维护；如果未来需要英文 release notes，可从本文件同步整理。

## [1.1.0] - 2026-07-26

### Added

- 内置 QuickJS Provider runtime：官方 Provider 不再依赖用户本机的 Node.js；Provider 作者可使用受控的 `qb` API 读取已声明的环境变量、文件及 HTTP 接口。
- 新增完整的 `builtin-js` Provider 作者文档、CLI 说明和示例，包含权限声明、执行模型、输出协议与支持边界。

### Changed

- 所有维护中的官方 Provider（Kimi、智谱 Coding Plan、DeepSeek、Codex Usage、光阴似箭）均已迁移到 `builtin-js`。
- Provider 运行期代理遵循应用的显式代理策略；未选择“系统代理”时不会隐式读取系统 HTTP(S) 代理环境变量。

### Security

- `builtin-js` 按 manifest 权限限制环境变量、文件与网络访问，保留 `QBWIN_*` 宿主变量给平台内部使用。

## [1.0.7] - 2026-07-23

### Fixed

- 修复托盘弹窗未保存手动尺寸时，每次从托盘重新唤起都会将窗口外框尺寸当作内容区尺寸恢复，导致宽高持续增大的问题。

## [1.0.6] - 2026-07-22

### Fixed

- 主界面的 GitHub 入口与更新区的“查看发布说明”改为通过 Windows 默认浏览器打开，避免 Tauri WebView 中的外部链接无响应。
- 更新设置页会在载入时直接显示当前应用版本，无需先执行更新检查。

## [1.0.5] - 2026-07-22

### Changed

- 更新应用图标。

### Fixed

- 修复从托盘打开弹窗时未始终根据本次托盘点击位置定位的问题。

## [1.0.4] - 2026-07-21

### Added

- 新增由 DeskFoundry `desktop-updater` 提供的 portable 应用更新基础：已签名 GitHub Raw 更新清单、GitHub Release ZIP 下载、SHA-256 校验、独立 helper 替换与启动确认回滚。
- 设置页新增“应用更新”检查入口；发现新版后可执行“下载并重启更新”。

### Security

- 应用更新仅接受 Ed25519 签名清单，且 ZIP 只允许覆盖发布包白名单中的可执行文件；portable marker、配置、日志、secrets、Provider 缓存和用户文件都会保留。

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
