# V5 Productization & Distribution Spec

> 目标：把 QuotaBarWin 从个人工具打磨成可分发、可诊断、可升级的软件。

---

## 1. 版本定位

V5 聚焦工程化，不新增 provider 能力。

---

## 2. 必须实现

### 2.1 构建与发布

- GitHub Actions 构建 Windows 安装包。
- 生成 release artifact。
- 产出 portable zip。
- README 写清安装和卸载方式。

### 2.2 自动更新

- 接入 Tauri updater。
- 支持检查更新。
- 设置页展示当前版本。
- 更新失败不影响主应用使用。

### 2.3 配置迁移

引入 config schema migration：

```ts
schemaVersion: 1 -> 2 -> 3
```

要求：

- 迁移前备份原配置。
- 迁移失败回滚。
- migration 有单元测试。

### 2.4 诊断包导出

导出 zip，包含：

```text
app version
platform info
redacted config
last snapshot
redacted diagnostics
recent logs
```

不得包含：

```text
API key
token
Authorization header
Cookie
完整本机用户名路径中的敏感段，如需展示应做最小化处理
```

### 2.5 日志

- 结构化日志。
- 日志分级。
- 日志轮转或大小限制。
- redaction middleware。

### 2.6 开机启动

如果 V1.1 未完成 autostart，此版本必须补齐。

---

## 3. 自动验收标准

Codex 必须自己跑：

```bash
npm run build
npm run test -- --run
cargo test --manifest-path src-tauri/Cargo.toml
```

如果环境支持：

```bash
npm run tauri build
```

### 3.1 Tests

至少包含：

```text
config_migration_v1_to_v2
config_migration_backup_created
config_migration_rolls_back_on_failure
diagnostics_export_redacts_secrets
logs_are_redacted_before_write
updater_config_is_present
release_workflow_yaml_is_valid
```

---

## 4. 完成定义

V5 完成必须满足：

- Windows installer / portable artifact 的构建配置存在。
- 自动更新配置存在。
- 配置迁移有测试。
- 诊断包导出有测试。
- 日志脱敏有测试。
- 不新增 provider，不修改 provider 协议语义。

---

## 5. 给 Codex 的短 Prompt

```text
目标版本：V5。实现产品化：GitHub Actions 构建、Windows installer/portable、Tauri updater 配置、config migration、diagnostics zip、结构化日志和脱敏测试。不要新增 provider。
```
