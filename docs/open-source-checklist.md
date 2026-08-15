# 开源发布审计

本文记录 QuotaBarWin 开源仓和 Release 的持续检查项。默认以 README、当前
公开文档和源代码为事实来源。

## 已建立的公开基础

- 使用 MIT License，npm 和 Cargo 元数据包含 license、repository、homepage
  和 issues 地址。
- 提供 `CONTRIBUTING.md`、`SECURITY.md`、`CODE_OF_CONDUCT.md`、Issue
  模板和 PR 模板。
- CI 使用 Windows、Node.js 22 和 Rust stable 执行前端构建、前后端测试及
  图标检查。
- Release workflow 构建 portable zip，同时包含 GUI、CLI、Updater 和
  `quotabarwin.portable` 标记。
- Release notes 从当前版本对应的 `CHANGELOG.md` 条目生成。
- Release notes 只说明相对上一已发布版本的用户可见差异，不记录内部迭代、
  预览开关、测试构建过程或无需用户处理的实现细节。
- portable update 使用独立 Ed25519 密钥；私钥只存放在 GitHub Actions
  secret，公钥通过仓库变量编译进客户端。
- 基础 CSP 已启用，日志、诊断和 Provider stderr 经过脱敏。
- `.gitignore` 覆盖常见私钥、证书、keystore、日志、配置和本地 secret。
- Provider manifest/source 和 registry checksum 由 Rust 测试校验。
- 当前 E2E 使用 schema 17 remote Provider fixture，覆盖设置保存、向导式
  Provider 保存/测试/启用、secret 不进入 config/log、Provider 刷新/超时、
  托盘交互和删除托管 secret。

## 每次公开同步前

- [ ] 对比私有候选分支与开源最新 `main`，确认没有遗漏公开仓新增提交。
- [ ] 只同步公开代码、测试和用户/开发者文档，不同步 `docs/internal/` 或内部
  实施规格。
- [ ] `git diff --check` 通过。
- [ ] 没有真实 API key、token、cookie、账号 ID、授权头、代理凭据、私钥、
  本地配置、日志或 diagnostics。
- [ ] 中文公开文档和已有英文对应文档同步。
- [ ] manifest `checksums.source` 和 registry manifest checksum 同步。
- [ ] package、Cargo、Tauri、README 和 CHANGELOG 版本一致。
- [ ] `AGENTS.md` 中的配置 schema、Provider contract 和 E2E 说明与代码一致。
- [ ] 应用内置 `src-tauri/src/remote_provider_guide.html` 与公开 Provider
  指南包含相同的关键契约和安全说明。

## 每个 Release 候选必须执行

```powershell
npm ci
npm run build
npm run test -- --run
cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1
npm run icons:check
npm run tauri -- build --no-bundle
npm run e2e
npm audit --audit-level=high
cargo audit --file src-tauri/Cargo.lock
```

还需人工验证：

- [ ] 全新 portable 目录可安装 Provider、填写表单、保存并测试、自动启用。
- [ ] 测试失败保留配置但不启用 Provider，重试成功后恢复。
- [ ] 旧配置升级后 Provider 保持 ready，不要求迁移旧 secret。
- [ ] 托管 secret 不出现在 config、日志、错误、diagnostics 或界面回显。
- [ ] HTTP、SOCKS5、系统环境代理和代理检测符合文档。
- [ ] GUI、CLI 和 Updater 均包含在最终 portable zip。
- [ ] GitHub Release 资产名、版本、CHANGELOG 和 tag 一致。
- [ ] Release notes 仅包含相对上一版本的用户可见变化，无内部迭代信息。
- [ ] 旧版客户端可通过正式更新通道升级到新版本。

## GitHub 仓设置

- [ ] Security Advisories 和 private vulnerability reporting 已启用。
- [ ] `QUOTABARWIN_UPDATE_PUBLIC_KEY` 仓库变量已配置。
- [ ] `DESKTOP_UPDATE_PRIVATE_KEY` Actions secret 已配置，且与公钥匹配。
- [ ] 发布前在最终公开提交上手工运行 CI workflow。
- [ ] 只从最终公开 `main` 创建版本 tag，不移动或覆盖已发布 tag。

## 依赖审计说明

网络错误、registry 不可达或 advisory database 拉取失败都不算审计通过。
必须在网络恢复后重新执行并保存成功结果。无漏洞但存在 warning 时，应在 PR
或 Release 记录其来源、适用平台和接受理由。
