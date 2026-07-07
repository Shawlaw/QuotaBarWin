# 开源准备审计

本文记录开源前需要补齐或持续关注的事项。默认以当前代码、README 和公开 Provider 文档为准；`docs/specs/` 仅作历史设计参考。

## 已处理

- 许可证：补充 MIT `LICENSE`，并在 `package.json`、`src-tauri/Cargo.toml` 中声明 license。
- 项目元数据：补充 npm/Cargo repository、homepage、bugs、Node 版本要求和贡献者作者信息。
- 人工确认：MIT 是最终许可证，`https://github.com/Shawlaw/QuotaBarWin` 是最终公开仓库地址。
- 协作入口：补充 `CONTRIBUTING.md`、`SECURITY.md`、`CODE_OF_CONDUCT.md`。
- GitHub 入口：补充 issue 模板、PR 模板、CI 工作流和 Dependabot 配置。
- 变更记录：补充 `CHANGELOG.md`，为公开 release 提供用户可见变更入口。
- 依赖安全：修复 `npm audit` 暴露的 `undici` 高危链路，并用 npm override 避开 `esbuild` 低危范围。
- Rust 依赖安全：安装并运行 `cargo audit`，更新 `src-tauri/Cargo.lock` 修复 `quick-xml` 和 `quinn-proto` 高危漏洞。
- 供应链检查：CI 运行 `npm audit --audit-level=low`，PR 运行 GitHub dependency review 覆盖 npm/Cargo lockfile。
- Tauri 安全姿态：启用基础 CSP，保留必要的 Tauri IPC source。
- 防误提交：扩展 `.gitignore`，覆盖常见签名证书、私钥和 keystore 文件。
- E2E：`e2e/tauri.e2e.mjs` 已改为生成 schema 14 remote Provider 配置和本地 cache；README 说明了单实例限制和临时产物保留开关。

## 开源前人工确认

- 确认 GitHub 仓库已启用 Security Advisories / private vulnerability reporting。
- 确认 v1.0.0 首个公开 Release 的 tag、zip 名称、截图和 README 描述一致。
- 确认本地 `.tmp/`、`dist/`、`node_modules/`、`src-tauri/target/` 等忽略目录不会被手动上传到 Release 或附件。

## 后续优化

- `cargo audit` 当前仍报告 GTK/Tauri Linux 链路、`proc-macro-error` 和 `unic-*` 的 warning；审计退出码为 0，无 remaining vulnerability。
