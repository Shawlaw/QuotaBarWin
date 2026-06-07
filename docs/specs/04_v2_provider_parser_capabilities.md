# V2 Provider Parser Capabilities Spec

> 目标：增强 Provider 内部解析能力，让 command provider 能把真实世界的 JSON 输出归一化成 ProviderSnapshot。V2 不做 preset UI，不要求真实调用外部服务。

---

## 1. 版本定位

V2 的关键词是：**parser 是 provider 内部实现能力**。

```text
CommandProvider
  -> RawCommandResult
  -> internal parser
  -> ProviderSnapshot
```

用户不应该在主 UI 里看到“adapter”这个概念。设置页可以有高级配置，但产品概念仍然是 Provider。

---

## 2. 必须实现的 ParserSpec

V2 在 V1 基础上增加：

```ts
export type ParserSpec =
  | { type: "app-snapshot" }
  | { type: "provider-snapshot" }
  | { type: "kimi-coding-usage-v1" }
  | { type: "bigmodel-quota-limit-json-v1" }
  | { type: "json-mapping"; mapping: JsonMappingSpec }
  | { type: "regex-blocks"; rules: RegexBlockRule[] };
```

其中本版本必须完整验收：

```text
kimi-coding-usage-v1
bigmodel-quota-limit-json-v1
```

`json-mapping` 和 `regex-blocks` 可以先做最小能力，满足 fixture 测试即可。

---

## 3. ProviderSnapshot metadata

V2 起允许 ProviderSnapshot 增加 metadata 字段，用来存放 UI 不直接渲染为 quota window、但对诊断或后续策略有价值的信息。

```ts
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
```

要求：

- UI 第一阶段可以不展示 metadata。
- metadata 不得包含明文 token、cookie、authorization header。
- metadata 必须可 JSON serialize。

---

## 4. Kimi Coding Usage JSON Parser

### 4.1 输入示例

真实调用形式类似：

```bash
curl -s -H "Authorization: Bearer <你的API_KEY>" \
  https://api.kimi.com/coding/v1/usages
```

响应 fixture：`fixtures/provider_outputs/kimi_coding_usage.json`

```json
{
  "user": {
    "userId": "d7inujlgsoa1c9h3en10",
    "region": "REGION_CN",
    "membership": { "level": "LEVEL_INTERMEDIATE" },
    "businessId": ""
  },
  "usage": {
    "limit": "100",
    "used": "15",
    "remaining": "85",
    "resetTime": "2026-06-12T02:35:14.207781Z"
  },
  "limits": [
    {
      "window": { "duration": 300, "timeUnit": "TIME_UNIT_MINUTE" },
      "detail": {
        "limit": "100",
        "used": "10",
        "remaining": "90",
        "resetTime": "2026-06-07T16:35:14.207781Z"
      }
    }
  ],
  "parallel": { "limit": "20" },
  "totalQuota": { "limit": "100", "remaining": "99" },
  "authentication": { "method": "METHOD_API_KEY", "scope": "FEATURE_CODING" },
  "subType": "TYPE_PURCHASE"
}
```

V2 验收必须只使用 fixture，不得调用真实接口。

### 4.2 期望输出

假设 provider config：

```json
{
  "id": "kimi-coding",
  "name": "Kimi Coding",
  "enabled": true,
  "kind": "command",
  "command": {
    "executable": "node",
    "args": ["fixtures/commands/kimi_usage_fixture.js"],
    "timeoutMs": 15000
  },
  "parser": { "type": "kimi-coding-usage-v1" }
}
```

Parser 应输出一个 ProviderSnapshot，等价于 `fixtures/expected/kimi_provider_snapshot.json`：

```json
{
  "id": "kimi-coding",
  "name": "Kimi Coding",
  "status": "ok",
  "source": "command",
  "windows": [
    {
      "id": "usage",
      "label": "Coding usage",
      "used": 15,
      "limit": 100,
      "usedPercent": 15,
      "remainingPercent": 85,
      "resetAt": "2026-06-12T02:35:14.207781Z",
      "confidence": "exact"
    },
    {
      "id": "300-minute",
      "label": "5h",
      "used": 10,
      "limit": 100,
      "usedPercent": 10,
      "remainingPercent": 90,
      "resetAt": "2026-06-07T16:35:14.207781Z",
      "confidence": "exact"
    },
    {
      "id": "total-quota",
      "label": "Total quota",
      "used": 1,
      "limit": 100,
      "usedPercent": 1,
      "remainingPercent": 99,
      "resetAt": null,
      "confidence": "exact"
    }
  ],
  "metadata": {
    "region": "REGION_CN",
    "membershipLevel": "LEVEL_INTERMEDIATE",
    "authMethod": "METHOD_API_KEY",
    "authScope": "FEATURE_CODING",
    "subType": "TYPE_PURCHASE",
    "parallelLimit": 20
  }
}
```

比较时允许 `updatedAt` 动态，percent 允许 0.01 误差。

### 4.3 解析规则

- `usage.used / usage.limit / usage.remaining / usage.resetTime` 生成主 window。
- 主 window label 固定为 `Coding usage`，id 固定为 `usage`。
- `limits[*].window.duration + timeUnit` 生成人类可读 label：
  - `duration=300, timeUnit=TIME_UNIT_MINUTE` -> `5h`
  - `duration=60, timeUnit=TIME_UNIT_MINUTE` -> `1h`
  - 无法识别时使用原始值，例如 `300 TIME_UNIT_MINUTE`。
- `limits[*].detail.used / limit / remaining / resetTime` 各生成一个 window。
- `totalQuota.limit / totalQuota.remaining` 生成 `Total quota` window；`used = limit - remaining`。
- `parallel.limit` 不生成 quota window，写入 `metadata.parallelLimit`。
- `user.region`、`user.membership.level`、`authentication.method`、`authentication.scope`、`subType` 写入 metadata。
- 数字字段可能是字符串，必须安全解析。
- 当 used/limit/remaining 三者同时存在时，优先使用服务端 remaining 计算 remainingPercent。
- 当只存在 used/limit 时，`remainingPercent = 100 - usedPercent`。
- `limit <= 0` 时 percent 为 null，status 至少为 warning。
- 所有 percent clamp 到 `[0, 100]`。
- 任意单个 window 字段异常不应导致整个 app 崩溃；应返回 warning diagnostics。

### 4.4 边界用例

Codex 应补测试：

```text
usage missing
usage.limit = 0
numeric strings with whitespace
limits empty
limits[*].detail missing
used > limit
remaining > limit
negative used
unknown timeUnit
invalid resetTime
totalQuota missing
parallel.limit missing
```

---

## 5. BigModel / Z.ai Quota JSON Parser

### 5.1 输入示例

真实调用形式类似：

```bash
curl -s -H "Authorization: Bearer YOUR_API_KEY" \
  "https://open.bigmodel.cn/api/monitor/usage/quota/limit"
```

响应 fixture：`fixtures/provider_outputs/bigmodel_quota_limit.json`

```json
{
  "code": 200,
  "msg": "操作成功",
  "data": {
    "limits": [
      {
        "type": "TIME_LIMIT",
        "unit": 5,
        "number": 1,
        "usage": 1000,
        "currentValue": 35,
        "remaining": 965,
        "percentage": 3,
        "nextResetTime": 1782784883993,
        "usageDetails": [
          { "modelCode": "search-prime", "usage": 32 },
          { "modelCode": "web-reader", "usage": 3 },
          { "modelCode": "zread", "usage": 0 }
        ]
      },
      {
        "type": "TOKENS_LIMIT",
        "unit": 3,
        "number": 5,
        "percentage": 1,
        "nextResetTime": 1780864083180
      },
      {
        "type": "TOKENS_LIMIT",
        "unit": 6,
        "number": 1,
        "percentage": 4,
        "nextResetTime": 1781316083997
      }
    ],
    "level": "pro"
  },
  "success": true
}
```

V2 验收必须只使用 fixture，不得调用真实接口。

### 5.2 期望输出

假设 provider config：

```json
{
  "id": "bigmodel-coding-plan",
  "name": "BigModel / Z.ai Coding Plan",
  "enabled": true,
  "kind": "command",
  "command": {
    "executable": "node",
    "args": ["fixtures/commands/bigmodel_quota_fixture.js"],
    "timeoutMs": 15000
  },
  "parser": { "type": "bigmodel-quota-limit-json-v1" }
}
```

Parser 应输出一个 ProviderSnapshot，等价于 `fixtures/expected/bigmodel_provider_snapshot.json`：

```json
{
  "id": "bigmodel-coding-plan",
  "name": "BigModel / Z.ai Coding Plan",
  "status": "ok",
  "source": "command",
  "windows": [
    {
      "id": "time-limit-5-1",
      "label": "TIME_LIMIT unit=5 number=1",
      "used": 35,
      "limit": 1000,
      "usedPercent": 3,
      "remainingPercent": 97,
      "resetAt": "2026-06-30T02:01:23.993000Z",
      "confidence": "exact"
    },
    {
      "id": "tokens-limit-3-5",
      "label": "TOKENS_LIMIT unit=3 number=5",
      "used": null,
      "limit": null,
      "unit": "tokens",
      "usedPercent": 1,
      "remainingPercent": 99,
      "resetAt": "2026-06-07T20:28:03.180000Z",
      "confidence": "exact"
    },
    {
      "id": "tokens-limit-6-1",
      "label": "TOKENS_LIMIT unit=6 number=1",
      "used": null,
      "limit": null,
      "unit": "tokens",
      "usedPercent": 4,
      "remainingPercent": 96,
      "resetAt": "2026-06-13T02:01:23.997000Z",
      "confidence": "exact"
    }
  ],
  "metadata": {
    "level": "pro",
    "usageDetails": {
      "search-prime": 32,
      "web-reader": 3,
      "zread": 0
    },
    "rawCode": 200,
    "rawMsg": "操作成功"
  }
}
```

比较时允许 `updatedAt` 动态，percent 允许 0.01 误差。

### 5.3 解析规则

- 响应必须是 JSON。
- `success !== true` 或 `code !== 200` 时，provider status = error。
- `data.level` 写入 `metadata.level`。
- 遍历 `data.limits[*]`，每一项生成一个 window。
- window id：`type` 转小写并把 `_` 替换为 `-`，再拼接 `unit` 和 `number`，例如 `tokens-limit-3-5`。
- window label：默认使用 `${type} unit=${unit} number=${number}`。不要擅自把 `unit` 数字解释为小时/月，除非后续有官方映射。
- 对于包含 `currentValue` 与 `usage` 的 limit：
  - `used = currentValue`
  - `limit = usage`
  - 如存在 `remaining`，可用于校验，但 `remainingPercent` 默认使用 `100 - percentage`。
- 对于只有 `percentage` 的 limit：
  - `used = null`
  - `limit = null`
  - `usedPercent = percentage`
  - `remainingPercent = 100 - percentage`
- `type === TOKENS_LIMIT` 时 `unit = "tokens"`；其他类型 unit 可以为 null。
- `nextResetTime` 是 Unix epoch milliseconds，必须转成 ISO UTC 字符串。
- `usageDetails` 不生成 window，归并成 `metadata.usageDetails`。
- 所有 percent clamp 到 `[0, 100]`。
- 字段缺失时尽量降级为 warning，不应导致 app 崩溃。

### 5.4 边界用例

Codex 应补测试：

```text
success=false
code != 200
data missing
limits missing or empty
limit item missing type
percentage missing
percentage > 100
percentage < 0
currentValue > usage
remaining inconsistent with usage-currentValue
nextResetTime missing
nextResetTime seconds instead of milliseconds
usageDetails empty
unknown type
```

---

## 6. Parser 测试要求

### 6.1 Fixture files

必须存在：

```text
fixtures/provider_outputs/kimi_coding_usage.json
fixtures/provider_outputs/bigmodel_quota_limit.json
fixtures/expected/kimi_provider_snapshot.json
fixtures/expected/bigmodel_provider_snapshot.json
```

### 6.2 自动验收命令

Codex 必须自己跑：

```bash
npm run build
npm run test -- --run
cargo test --manifest-path src-tauri/Cargo.toml
```

### 6.3 Rust tests

必须包含：

```text
parse_kimi_coding_usage_fixture
parse_bigmodel_quota_limit_json_fixture
parse_kimi_invalid_json_returns_error
parse_bigmodel_unsuccessful_returns_error
parse_percentage_clamps_to_0_100
parse_epoch_ms_to_iso_utc
redacts_authorization_header
```

### 6.4 TypeScript tests

如果 parser 在 TS 侧实现，必须包含同等测试。若 parser 在 Rust 侧实现，TS 至少测试 UI 渲染 fixture snapshot。

---

## 7. Codex 自主验收要求

Codex 完成 V2 后必须输出机器可读摘要：

```json
{
  "version": "V2",
  "completed": true,
  "tests": {
    "npmBuild": "pass",
    "npmTest": "pass",
    "cargoTest": "pass",
    "kimiParserFixture": "pass",
    "bigmodelJsonParserFixture": "pass"
  },
  "notes": []
}
```

如果失败，Codex 必须先自修复，再重新跑验收。最多三轮自修复；三轮后输出失败原因、失败命令、相关日志摘要。

---

## 8. 给 Codex 的低上下文任务语句

```text
目标版本：V2。增强 Provider 内部 parser 能力，必须支持 kimi-coding-usage-v1 JSON parser 和 bigmodel-quota-limit-json-v1 JSON parser，并用 fixture 自验收。不要调用真实接口，不要把 adapter 变成 UI 概念，不要新增 preset UI。
```
