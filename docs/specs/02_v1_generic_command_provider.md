# V1 Generic Command Provider MVP Spec

> 目标：把 V0 变成第一个可日常使用的版本。V1 的关键不是接某个具体服务，而是实现“Provider 可以通过命令获取结果，并输出标准 ProviderSnapshot”。

---

## 1. 版本定位

V1 是通用命令型 provider 的 MVP。

核心链路：

```text
CommandProvider
  -> Command::new(executable).args(args)
  -> RawCommandResult
  -> parser: app-snapshot | provider-snapshot
  -> ProviderSnapshot / AppSnapshot
  -> UI render
```

V1 不实现 Kimi / BigModel 专用 parser。它们在 V2 实现。

---

## 2. 必须实现

### 2.1 本地配置

配置路径：

- Windows: `%APPDATA%/QuotaBarWin/config.json`
- 其他平台：使用 Tauri app config dir

默认配置：

```json
{
  "schemaVersion": 1,
  "refreshIntervalSeconds": 300,
  "displayMode": "remaining",
  "lowQuotaWarningThreshold": 20,
  "providers": [
    {
      "id": "mock-codex",
      "name": "Codex Mock",
      "enabled": true,
      "kind": "mock"
    }
  ]
}
```

配置损坏时：

- 将损坏文件备份为 `config.corrupt.<timestamp>.json`。
- 重建默认配置。
- diagnostics 中记录恢复事件。

### 2.2 ProviderConfig

```ts
export type ProviderConfig =
  | {
      id: string;
      name: string;
      enabled: boolean;
      kind: "mock";
    }
  | {
      id: string;
      name: string;
      enabled: boolean;
      kind: "command";
      command: CommandSpec;
      parser: ParserSpec;
    };
```

### 2.3 CommandSpec

```ts
export type CommandSpec = {
  executable: string;
  args: string[];
  cwd?: string | null;
  env?: Record<string, string>;
  timeoutMs: number;
};
```

执行要求：

- Rust 使用 `Command::new(executable).args(args)`。
- 不支持 shell 字符串。
- 不得使用 `shell=true`。
- 超时后必须终止子进程。
- 捕获 stdout、stderr、exitCode、durationMs、timedOut。
- 单个 command provider 失败不能影响其他 provider。

### 2.4 V1 ParserSpec

V1 只实现两种 parser：

```ts
export type ParserSpec =
  | { type: "app-snapshot" }
  | { type: "provider-snapshot" };
```

#### app-snapshot

命令 stdout 直接输出：

```json
{
  "schemaVersion": 1,
  "providers": [
    {
      "id": "external-codex",
      "name": "External Codex",
      "status": "ok",
      "source": "command",
      "updatedAt": "2026-06-08T10:00:00+08:00",
      "windows": []
    }
  ],
  "refreshedAt": "2026-06-08T10:00:00+08:00"
}
```

#### provider-snapshot

命令 stdout 直接输出单个 provider：

```json
{
  "id": "external-codex",
  "name": "External Codex",
  "status": "ok",
  "source": "command",
  "updatedAt": "2026-06-08T10:00:00+08:00",
  "windows": [
    {
      "id": "weekly",
      "label": "Weekly",
      "used": 123,
      "limit": 1000,
      "unit": "requests",
      "usedPercent": 12.3,
      "remainingPercent": 87.7,
      "resetAt": null,
      "confidence": "exact"
    }
  ]
}
```

### 2.5 Rust commands

必须暴露：

```rust
get_config() -> Result<AppConfig, String>
save_config(config: AppConfig) -> Result<(), String>
refresh_snapshot() -> Result<AppSnapshot, String>
get_cached_snapshot() -> Result<Option<AppSnapshot>, String>
```

前端不得直接读写配置文件，不得直接执行命令。

### 2.6 刷新行为

- 启动后立即刷新一次。
- 点击 Refresh 手动刷新。
- 按 `refreshIntervalSeconds` 定时刷新。
- 正在刷新时，新的刷新请求不得并发执行。
- 刷新失败时保留上一次成功 snapshot。
- disabled provider 不执行。

### 2.7 UI

主面板展示：

- 顶部标题：QuotaBarWin
- 最后刷新时间
- Refresh 按钮
- provider card list
- error state
- Settings 入口

设置页支持：

- `refreshIntervalSeconds`
- `displayMode`
- `lowQuotaWarningThreshold`
- provider 列表展示
- 新增/编辑 command provider 的基本字段：name、executable、args、timeout、parser type

---

## 3. 安全要求

### 3.1 日志脱敏

日志和 diagnostics 不得输出：

- `Authorization: Bearer ...`
- `API_KEY`
- `TOKEN`
- `Cookie`
- `Set-Cookie`
- 任意看起来像长 token 的值

必须实现 redaction helper，并有单元测试。

### 3.2 API Key 存储

V1 不实现 secret store。配置中可以支持环境变量引用，但不得鼓励写明文 token。

建议格式：

```json
{
  "env": {
    "KIMI_API_KEY": "${env:KIMI_API_KEY}"
  }
}
```

V1 可以只保留结构，不强制实现完整 secret 管理。

---

## 4. 自动验收标准

Codex 必须自己跑：

```bash
npm install
npm run build
npm run test -- --run
cargo test --manifest-path src-tauri/Cargo.toml
```

### 4.1 Rust tests

至少包含：

```text
loads_default_config_when_missing
backs_up_corrupt_config
command_provider_parses_provider_snapshot_stdout
command_provider_parses_app_snapshot_stdout
command_provider_handles_non_zero_exit
command_provider_handles_timeout
command_provider_does_not_execute_disabled_provider
redacts_authorization_and_tokens_from_logs
percentages_are_clamped_to_0_100
```

### 4.2 Frontend tests

至少包含：

```text
settings_can_render_command_provider
provider_card_renders_error_state
provider_card_renders_used_and_remaining
refresh_button_calls_refresh_snapshot
```

### 4.3 Fake command 验收

Codex 必须创建或使用 fake command fixture，例如：

```bash
node fixtures/fake_provider_snapshot.js
```

它输出合法 ProviderSnapshot。测试应验证 command provider 可以解析它。

不得调用真实外部网络接口。

---

## 5. 完成定义

V1 完成必须满足：

- mock provider 仍然正常。
- command provider 可以执行本地 fake command。
- app-snapshot 和 provider-snapshot 两种 stdout 都能解析。
- command 失败、超时、JSON 错误不会导致 app 崩溃。
- 配置可以保存并重启后生效。
- 关闭窗口隐藏到托盘，Quit 才退出。
- 自动测试通过。

---

## 6. 给 Codex 的短 Prompt

```text
目标版本：V1。实现通用 command provider MVP：配置文件、settings、Command::new 执行、app-snapshot/provider-snapshot parser、定时刷新、缓存、错误隔离、日志脱敏和自动测试。不要实现 Kimi/BigModel 专用 parser，不要接真实网络接口。
```
