# Provider Examples & Fixtures

> 本文件把用户给出的两个实际 provider 数据样例固化为设计输入、fixture 和期望归一化结果。Codex 实现 V2/V3 时应以这里为准做自验收。

---

## 1. Provider 抽象回顾

对 UI 来说，所有 provider 都应该长得一样：

```text
Provider.provide() -> ProviderSnapshot
```

但 provider 内部可以完全不同：

```text
Kimi provider
  -> curl JSON API
  -> parse kimi-coding-usage-v1
  -> ProviderSnapshot

BigModel/Z.ai provider
  -> curl JSON API
  -> parse bigmodel-quota-limit-json-v1
  -> ProviderSnapshot
```

Parser/adapter 是 provider 内部实现细节，不能成为 UI 或产品层概念。

---

## 2. Kimi Coding Usage Provider

### 2.1 实际命令形态

```bash
curl -s -H "Authorization: Bearer <你的API_KEY>" \
  https://api.kimi.com/coding/v1/usages
```

在 QuotaBarWin 中不要把 `| jq` 写进 command。应只执行 curl，parser 自己处理 JSON：

```json
{
  "executable": "curl",
  "args": [
    "-s",
    "-H",
    "Authorization: Bearer ${env:KIMI_API_KEY}",
    "https://api.kimi.com/coding/v1/usages"
  ],
  "timeoutMs": 15000
}
```

### 2.2 响应 fixture

文件：`fixtures/provider_outputs/kimi_coding_usage.json`

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

### 2.3 期望归一化 ProviderSnapshot

文件：`fixtures/expected/kimi_provider_snapshot.json`

```json
{
  "id": "kimi-coding",
  "name": "Kimi Coding",
  "status": "ok",
  "source": "command",
  "updatedAt": "<dynamic>",
  "windows": [
    {
      "id": "usage",
      "label": "Coding usage",
      "used": 15,
      "limit": 100,
      "unit": null,
      "usedPercent": 15,
      "remainingPercent": 85,
      "resetAt": "2026-06-12T02:35:14.207781Z",
      "resetText": null,
      "confidence": "exact"
    },
    {
      "id": "300-minute",
      "label": "5h",
      "used": 10,
      "limit": 100,
      "unit": null,
      "usedPercent": 10,
      "remainingPercent": 90,
      "resetAt": "2026-06-07T16:35:14.207781Z",
      "resetText": null,
      "confidence": "exact"
    },
    {
      "id": "total-quota",
      "label": "Total quota",
      "used": 1,
      "limit": 100,
      "unit": null,
      "usedPercent": 1,
      "remainingPercent": 99,
      "resetAt": null,
      "resetText": null,
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
  },
  "error": null
}
```

比较时允许 `updatedAt` 动态，percent 允许 0.01 误差。

### 2.4 边界用例

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

## 3. BigModel / Z.ai Coding Plan Provider

### 3.1 实际命令形态

```bash
curl -s -H "Authorization: Bearer YOUR_API_KEY" \
  "https://open.bigmodel.cn/api/monitor/usage/quota/limit"
```

在 QuotaBarWin 中建议写成：

```json
{
  "executable": "curl",
  "args": [
    "-s",
    "-H",
    "Authorization: Bearer ${env:BIGMODEL_API_KEY}",
    "https://open.bigmodel.cn/api/monitor/usage/quota/limit"
  ],
  "timeoutMs": 15000
}
```

### 3.2 响应 fixture

文件：`fixtures/provider_outputs/bigmodel_quota_limit.json`

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

### 3.3 期望归一化 ProviderSnapshot

文件：`fixtures/expected/bigmodel_provider_snapshot.json`

```json
{
  "id": "bigmodel-coding-plan",
  "name": "BigModel / Z.ai Coding Plan",
  "status": "ok",
  "source": "command",
  "updatedAt": "<dynamic>",
  "windows": [
    {
      "id": "time-limit-5-1",
      "label": "TIME_LIMIT unit=5 number=1",
      "used": 35,
      "limit": 1000,
      "unit": null,
      "usedPercent": 3,
      "remainingPercent": 97,
      "resetAt": "2026-06-30T02:01:23.993000Z",
      "resetText": null,
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
      "resetText": null,
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
      "resetText": null,
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
  },
  "error": null
}
```

比较时允许 `updatedAt` 动态，percent 允许 0.01 误差。

### 3.4 边界用例

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

## 4. Fixture 验收总表

```json
{
  "fixtures": {
    "kimiInput": "fixtures/provider_outputs/kimi_coding_usage.json",
    "kimiExpected": "fixtures/expected/kimi_provider_snapshot.json",
    "bigmodelInput": "fixtures/provider_outputs/bigmodel_quota_limit.json",
    "bigmodelExpected": "fixtures/expected/bigmodel_provider_snapshot.json"
  },
  "requiredParserTests": {
    "kimi-coding-usage-v1": "pass",
    "bigmodel-quota-limit-json-v1": "pass"
  }
}
```
