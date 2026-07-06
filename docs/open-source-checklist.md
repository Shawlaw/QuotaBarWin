# 开源准备审计

本文记录开源前需要补齐或持续关注的事项。默认以当前代码、README 和公开 Provider 文档为准；`docs/specs/` 仅作历史设计参考。

## 已处理

- 许可证：补充 MIT `LICENSE`，并在 `package.json`、`src-tauri/Cargo.toml` 中声明 license。
- 项目元数据：补充 npm/Cargo repository、homepage、bugs、Node 版本要求和贡献者作者信息。
- 协作入口：补充 `CONTRIBUTING.md`、`SECURITY.md`、`CODE_OF_CONDUCT.md`。
- GitHub 入口：补充 issue 模板、PR 模板、CI 工作流和 Dependabot 配置。
- 依赖安全：修复 `npm audit` 暴露的 `undici` 高危链路，并用 npm override 避开 `esbuild` 低危范围。
- Tauri 安全姿态：启用基础 CSP，保留必要的 Tauri IPC source。
- 防误提交：扩展 `.gitignore`，覆盖常见签名证书、私钥和 keystore 文件。

## 开源前人工确认

- 确认 MIT 是否为最终许可证；如果需要 Apache-2.0、GPL 或双许可证，应在公开前替换 `LICENSE` 和包元数据。
- 确认 GitHub 仓库已启用 Security Advisories / private vulnerability reporting。
- 确认 `https://github.com/Shawlaw/QuotaBarWin` 是最终公开仓库地址；如迁移组织名，需要同步 README、包元数据、默认远程 Provider registry URL 和托盘菜单链接。
- 确认 v1.0.0 首个公开 Release 的 tag、zip 名称、截图和 README 描述一致。
- 确认本地 `.tmp/`、`dist/`、`node_modules/`、`src-tauri/target/` 等忽略目录不会被手动上传到 Release 或附件。

## 后续优化

- 在 CI 中加入 Rust 依赖漏洞扫描，例如 `cargo audit` 或 GitHub dependency review。
- 维护 `CHANGELOG.md`，让每个 tag 的用户可见变化和迁移风险更清晰。
- E2E 当前历史上可能落后于 Provider schema；在公开宣传 E2E 可靠性前，应先刷新 `e2e/tauri.e2e.mjs` 的种子数据和运行说明。
