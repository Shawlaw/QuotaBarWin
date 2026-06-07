# V3 Provider Presets Spec

> 目标：在 Provider Runtime 已稳定后，提供常用 provider preset。Preset 是可编辑的 provider 配置模板，不是核心架构依赖。

---

## 1. 版本定位

V3 的目标是降低用户配置成本。

用户看到的是：

```text
Add Provider
  - Kimi Coding Usage
  - BigModel / Z.ai Coding Plan
  - OpenCode Quota Command
  - Custom Command Provider
```

用户不应看到：

```text
adapter registry
parser internals
opencode-quota as core dependency
```

---

## 2. Preset 模型

```ts
export type ProviderPreset = {
  id: string;
  displayName: string;
  description: string;
  providerConfigTemplate: ProviderConfig;
  requiredEnvVars?: string[];
  docs?: string;
};
```

要求：

- Preset 创建的是普通 `ProviderConfig`。
- 用户添加 preset 后可以编辑 name、command、args、timeout。
- API Key 默认通过环境变量读取。
- 不得把明文 token 写进默认配置。

---

## 3. 必须内置的 Presets

### 3.1 Kimi Coding Usage

真实命令方向：

```bash
curl -s -H "Authorization: Bearer <你的API_KEY>" \
  https://api.kimi.com/coding/v1/usages
```

Preset 配置模板：

```json
{
  "id": "kimi-coding",
  "name": "Kimi Coding",
  "enabled": true,
  "kind": "command",
  "command": {
    "executable": "curl",
    "args": [
      "-s",
      "-H",
      "Authorization: Bearer ${env:KIMI_API_KEY}",
      "https://api.kimi.com/coding/v1/usages"
    ],
    "timeoutMs": 15000
  },
  "parser": { "type": "kimi-coding-usage-v1" }
}
```

要求：

- UI 提示用户设置环境变量 `KIMI_API_KEY`。
- diagnostics 中不得显示 API Key。
- 验收使用 fixture，不调用真实 API。
- parser 必须支持新版 Kimi JSON：`usage`、`limits`、`parallel`、`totalQuota`、`authentication`、`subType`。

### 3.2 BigModel / Z.ai Coding Plan Usage

真实命令方向：

```bash
curl -s -H "Authorization: Bearer YOUR_API_KEY" \
  "https://open.bigmodel.cn/api/monitor/usage/quota/limit"
```

Preset 配置模板：

```json
{
  "id": "bigmodel-coding-plan",
  "name": "BigModel / Z.ai Coding Plan",
  "enabled": true,
  "kind": "command",
  "command": {
    "executable": "curl",
    "args": [
      "-s",
      "-H",
      "Authorization: Bearer ${env:BIGMODEL_API_KEY}",
      "https://open.bigmodel.cn/api/monitor/usage/quota/limit"
    ],
    "timeoutMs": 15000
  },
  "parser": { "type": "bigmodel-quota-limit-json-v1" }
}
```

要求：

- UI 提示用户设置环境变量 `BIGMODEL_API_KEY`。
- 兼容新版 JSON 响应：`code`、`success`、`data.limits`、`data.level`。
- 不再把 BigModel/Z.ai 主 preset 设计成文本 block parser。
- 验收使用 fixture，不调用真实 API。

### 3.3 OpenCode Quota Command

Preset 配置模板：

```json
{
  "id": "opencode-quota",
  "name": "OpenCode Quota",
  "enabled": true,
  "kind": "command",
  "command": {
    "executable": "opencode-quota",
    "args": ["show", "--json"],
    "timeoutMs": 15000
  },
  "parser": { "type": "app-snapshot" }
}
```

注意：

- 不自动安装 opencode-quota。
- 不 vendor opencode-quota。
- 只检测命令是否存在，缺失时给出诊断。
- 它只是 preset，不是主线依赖。

### 3.4 Custom Command Provider

允许用户手工配置：

- name
- executable
- args
- timeout
- parser type

V3 中高级 parser 配置可以先做 JSON textarea，不必做复杂表单。

---

## 4. UI 要求

设置页新增：

```text
Providers
  Add Provider
    Presets
      Kimi Coding Usage
      BigModel / Z.ai Coding Plan
      OpenCode Quota Command
      Custom Command Provider
```

添加 preset 后：

- 展示 provider config 编辑页。
- 显示 required env vars。
- 提供 Test Provider 按钮。
- Test Provider 使用当前配置运行一次，并显示脱敏 diagnostics。

---

## 5. Secret 与环境变量

必须实现环境变量占位符解析：

```text
${env:KIMI_API_KEY}
${env:BIGMODEL_API_KEY}
```

规则：

- 执行命令前替换为当前进程环境变量。
- 如果环境变量不存在，provider status = error，error message 应提示缺失变量名。
- diagnostics 只显示变量名，不显示值。
- redaction helper 必须处理替换后的 args。

---

## 6. 自动验收标准

Codex 必须自己跑：

```bash
npm run build
npm run test -- --run
cargo test --manifest-path src-tauri/Cargo.toml
```

### 6.1 Rust tests

必须包含：

```text
preset_kimi_has_required_env_var
preset_bigmodel_has_required_env_var
preset_opencode_quota_uses_app_snapshot_parser
preset_add_creates_provider_config
missing_env_var_returns_error_without_secret
redaction_hides_authorization_bearer
fixture_kimi_preset_parses_expected_snapshot
fixture_bigmodel_preset_parses_expected_snapshot
```

### 6.2 UI tests

必须包含：

```text
settings_shows_add_provider
add_kimi_preset_shows_env_hint
add_bigmodel_preset_shows_env_hint
add_custom_command_provider
```

如果没有浏览器级 e2e，至少用 component/unit tests 验证 preset list 和表单状态。

---

## 7. GUI Smoke Checklist

Codex 若能运行桌面环境，应执行：

```text
- 打开 Settings
- 点击 Add Provider
- 添加 Kimi Coding Usage
- 看到 KIMI_API_KEY 提示
- 添加 BigModel / Z.ai Coding Plan
- 看到 BIGMODEL_API_KEY 提示
- 点击 Test Provider，在缺少 env 时显示 error 且不泄露 secret
```

若无 GUI 环境，必须输出无法执行 GUI smoke 的原因，并用测试覆盖替代。

---

## 8. 完成定义

V3 完成时必须满足：

- 可以从 UI 添加 Kimi preset。
- 可以从 UI 添加 BigModel/Z.ai preset。
- 可以从 UI 添加 OpenCode Quota preset。
- 可以添加 Custom Command Provider。
- env placeholder 可解析。
- 缺失 env 时不崩溃。
- diagnostics 不泄露 secret。
- Preset 只是生成 ProviderConfig，不改变 Provider Runtime 架构。
- opencode-quota 只是 preset，不是核心依赖。

---

## 9. Codex 自主验收输出

Codex 完成后必须输出：

```json
{
  "version": "V3",
  "completed": true,
  "tests": {
    "npmBuild": "pass",
    "npmTest": "pass",
    "cargoTest": "pass",
    "presetTests": "pass",
    "secretRedactionTests": "pass"
  },
  "guiSmoke": "pass|skipped-with-reason",
  "notes": []
}
```

---

## 10. 给 Codex 的低上下文任务语句

```text
目标版本：V3。实现 Provider Presets：Kimi Coding Usage、BigModel/Z.ai Coding Plan、OpenCode Quota Command、Custom Command。Preset 只是生成 ProviderConfig，不是核心依赖。实现 env placeholder 和 Test Provider。验收必须用 fixture，不得调用真实 API。
```
