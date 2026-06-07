# QuotaBarWin 全景规划与编码契约

> 项目目标：开发一个 WindowsFirst 的 AI Coding / Agent CLI 用量查询显示小工具。它先作为系统托盘 quota monitor 存在，后续演进成 CommandHub / 多 Agent 工作台的额度调度基础设施。

---

## 1. 核心抽象：Provider 是唯一对外数据提供者

本项目的核心抽象不是 adapter，不是 command，也不是某个具体服务。

**Provider 是唯一对外的数据提供抽象。**

```text
Provider.provide() -> ProviderSnapshot
```

Provider 内部可以用任何方式获取和解析数据：

```text
Provider
  ├─ 执行 curl / CLI / PowerShell / Node / Python / 自定义 exe
  ├─ 读取本地文件 / 本地 CLI 配置 / 本地缓存
  ├─ 调用 native Rust / sidecar / external command
  ├─ 解析 JSON / 文本 / 正则 / 内置格式
  └─ 归一化为 ProviderSnapshot
```

UI、托盘、预测、通知、CommandHub 集成等上层逻辑只能依赖标准化后的 `ProviderSnapshot` / `AppSnapshot`。

---

## 2. command 是 Provider 的默认数据获取方式

为了最大通用性，第一阶段所有真实 usage 来源都应先降维成命令执行：

```text
执行一个命令
  -> 得到 RawCommandResult(stdout, stderr, exitCode, duration)
  -> Provider 内部 parser 解析
  -> ProviderSnapshot
```

这意味着以下数据源都可以统一接入：

```text
curl -s https://...
某个 CLI usage 命令
opencode-quota show --json
PowerShell 脚本
Node/Python/Bun 脚本
用户自写 exe
```

### 2.1 opencode-quota 的定位

`opencode-quota` 不再是架构中心。它只是一个 provider preset：

```json
{
  "id": "opencode-quota",
  "name": "OpenCode Quota",
  "kind": "command",
  "command": {
    "executable": "opencode-quota",
    "args": ["show", "--json"],
    "timeoutMs": 15000
  },
  "parser": { "type": "app-snapshot" }
}
```

---

## 3. 推荐技术栈

固定默认技术栈：

- Tauri 2
- React
- TypeScript
- Vite
- Rust backend
- 普通 CSS
- Vitest for TypeScript tests
- Rust unit tests for backend provider/config/parser logic

第一阶段禁止引入：

- Electron
- WPF / WinUI
- egui
- Redux / MobX / Zustand
- 数据库
- Tailwind
- 大型 UI 组件库
- 复杂图表库

---

## 4. 总体架构

```text
┌────────────────────────────────────┐
│ UI Layer                            │
│ Tray / Panel / Settings / Alerts    │
│ 只消费 AppSnapshot                  │
└─────────────────┬──────────────────┘
                  │
┌─────────────────▼──────────────────┐
│ Tauri Host Layer                    │
│ window / tray / config / cache      │
│ autostart / updater / notification  │
└─────────────────┬──────────────────┘
                  │
┌─────────────────▼──────────────────┐
│ Provider Runtime                    │
│ schedule / cache / concurrency      │
│ diagnostics / redaction / normalize │
└─────────────────┬──────────────────┘
                  │
┌─────────────────▼──────────────────┐
│ Provider Implementations            │
│ mock / command / native             │
│ fetch + parse + normalize internally│
└────────────────────────────────────┘
```

边界要求：

- UI 只认 `AppSnapshot`。
- Tauri Host 负责系统集成、窗口、托盘、配置、缓存、调度。
- Provider Runtime 负责 provider 调度、并发控制、错误隔离、诊断和日志脱敏。
- Provider Implementation 负责真实数据来源、内部解析和归一化。
- Parser/adapter 不应成为 UI 或产品层概念。

---

## 5. 统一数据协议

从 V1 开始使用以下结构，后续只允许向后兼容扩展，不允许破坏字段语义。

```ts
export type ProviderStatus = "ok" | "warning" | "error" | "unknown";
export type ConfidenceLevel = "exact" | "estimated" | "unknown";
export type ProviderSource = "mock" | "command" | "native";

export type QuotaWindow = {
  id: string;
  label: string;
  used: number | null;
  limit: number | null;
  unit?: string | null;
  usedPercent: number | null;
  remainingPercent: number | null;
  resetAt: string | null;
  resetText?: string | null;
  confidence: ConfidenceLevel;
};

export type ProviderDiagnostics = {
  checkedAt: string;
  messages: string[];
  commandPath?: string | null;
  exitCode?: number | null;
  durationMs?: number | null;
  timedOut?: boolean | null;
  stderr?: string | null;
};

export type ProviderSnapshot = {
  id: string;
  name: string;
  status: ProviderStatus;
  source: ProviderSource;
  updatedAt: string | null;
  windows: QuotaWindow[];
  error?: string | null;
  diagnostics?: ProviderDiagnostics | null;
  metadata?: Record<string, unknown> | null;
};

export type AppSnapshot = {
  schemaVersion: 1;
  providers: ProviderSnapshot[];
  refreshedAt: string;
};
```

说明：

- `used` 和 `limit` 是原始数值，便于后续展示 `0.5M / 10M`、`200 / 500`。
- `usedPercent` 和 `remainingPercent` 是 UI 渲染 progress bar 的标准字段。
- `resetAt` 是可解析的绝对时间；`resetText` 保留 `in 4 hours` 这类原文。
- 所有 percent 必须 clamp 到 `[0, 100]`。
- `remainingPercent = 100 - usedPercent`，除非数据源直接给出且更可信。

---

## 6. Provider 配置协议

```ts
export type AppConfig = {
  schemaVersion: 1;
  refreshIntervalSeconds: number;
  displayMode: "remaining" | "used";
  lowQuotaWarningThreshold: number;
  providers: ProviderConfig[];
};

export type ProviderConfig =
  | MockProviderConfig
  | CommandProviderConfig
  | NativeProviderConfig;

export type MockProviderConfig = {
  id: string;
  name: string;
  enabled: boolean;
  kind: "mock";
};

export type CommandProviderConfig = {
  id: string;
  name: string;
  enabled: boolean;
  kind: "command";
  command: CommandSpec;
  parser: ParserSpec;
};

export type NativeProviderConfig = {
  id: string;
  name: string;
  enabled: boolean;
  kind: "native";
  nativeProviderId: string;
  options?: Record<string, unknown>;
};
```

### 6.1 CommandSpec

```ts
export type CommandSpec = {
  executable: string;
  args: string[];
  cwd?: string | null;
  env?: Record<string, string>;
  timeoutMs: number;
};
```

安全要求：

- 默认只支持 `executable + args`。
- 不允许第一阶段支持 shell 字符串。
- Rust 必须使用 `Command::new(executable).args(args)`。
- 不得使用 `shell=true`。
- 不得拼接 shell 命令。
- 日志中必须脱敏 `Authorization`、`Bearer`、`API_KEY`、`TOKEN`、cookie 等敏感信息。
- API Key 默认通过环境变量引用，不写入 config。

### 6.2 ParserSpec 是 Provider 内部实现配置

虽然配置中会出现 `parser`，但它不是产品层的一等概念。它只是 command provider 内部的解析策略。

```ts
export type ParserSpec =
  | { type: "app-snapshot" }
  | { type: "provider-snapshot" }
  | { type: "kimi-coding-usage-v1" }
  | { type: "bigmodel-quota-limit-json-v1" }
  | { type: "json-mapping"; mapping: JsonMappingSpec }
  | { type: "regex-blocks"; rules: RegexBlockRule[] };
```

V1 只需要实现：

```text
app-snapshot
provider-snapshot
```

V2 再实现：

```text
kimi-coding-usage-v1
bigmodel-quota-limit-json-v1
json-mapping / regex-blocks 的最小可用能力
```

---

## 7. 版本路线

| 版本 | 名称 | 目标 |
|---|---|---|
| V0 | Technical Spike | 跑通 Tauri + tray + mock provider |
| V1 | Generic Command Provider MVP | command provider + direct snapshot parser + 本地配置 + 定时刷新 |
| V1.1 | Windows Polish | 启动隐藏、tooltip、通知、缓存、配置恢复、窗口位置记忆 |
| V2 | Provider Parser Capabilities | Kimi JSON / BigModel JSON parser、fixture 测试 |
| V3 | Provider Presets | Kimi、BigModel/Z.ai、opencode-quota、自定义 command preset |
| V4 | Forecast & Alerts | 瓶颈窗口、reset 倒计时、低额度预测、用量建议 |
| V5 | Productization | installer、自动更新、配置迁移、诊断包、发布流水线 |
| V6 | CommandHub Integration | quota 接入 Agent 启动、群聊、任务调度 |

---

## 8. 全局完成标准

任一版本完成时，Codex 必须输出机器可读摘要：

```text
版本：Vx
状态：pass | partial | blocked
变更摘要：
- ...
修改文件：
- ...
已运行命令：
- <command> => pass/fail
自动验收：
- ... => pass/fail
自修复记录：
- ...
未能自动验证：
- 项目：...
  原因：...
  替代验证：...
下一步：
- ...
```

完成前置条件：

- 自动测试通过。
- Provider parser fixture 测试通过。
- command 执行失败不会导致 app 崩溃。
- UI 可以从标准 `AppSnapshot` 渲染。
- 无敏感信息输出到日志。
