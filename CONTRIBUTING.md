# 贡献指南

QuotaBarWin 默认文档语言是简体中文；如果你修改已有英文文档的中文对应内容，也请同步更新英文版本。

## 开始之前

1. 先阅读 [AGENTS.md](AGENTS.md)、[README.md](README.md) 和相关模块附近的测试。
2. 确认你的改动符合核心约束：Provider 是唯一对外数据抽象，前端只消费 `AppSnapshot`、`ProviderSnapshot` 和 `QuotaWindow`。
3. 不要提交真实 API key、token、cookie、账号 ID、授权头、代理凭据、私钥、真实日志或诊断包。

## 本地开发

```powershell
npm install
npm run dev
npm run tauri dev
```

常用检查：

```powershell
npm run build
npm run test -- --run
cargo test --manifest-path src-tauri/Cargo.toml
```

如果改动涉及图标：

```powershell
npm run icons:check
```

## Pull Request 要求

- 说明用户可见行为、配置/迁移影响和测试结果。
- 前端改动通常需要 Vitest；Rust 后端改动需要对应模块的单元测试。
- 改远程 Provider 示例时，保持 `provider.json` 的 `checksums.source` 和 `examples/remote-providers/registry.json` 中的 checksum 同步。
- 改公开文档时，中文优先；已有英文对应文档时同步更新。
- 如果无法运行某个检查，请在 PR 中说明原因和剩余风险。

## English Summary

QuotaBarWin uses Simplified Chinese as the primary documentation language. Keep English counterparts in sync when they already exist. Do not commit secrets, real credentials, raw sensitive logs, or signing material. For normal changes, run:

```powershell
npm run build
npm run test -- --run
cargo test --manifest-path src-tauri/Cargo.toml
```
