# 变更记录

本文记录 QuotaBarWin 的用户可见变化。默认以简体中文维护；如果未来需要英文 release notes，可从本文件同步整理。

## [1.0.0] - 待发布

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
