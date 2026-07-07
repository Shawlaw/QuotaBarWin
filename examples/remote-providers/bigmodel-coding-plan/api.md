# BigModel Coding Plan API 依据

English version: [`api.en.md`](api.en.md).

本文记录 `bigmodel-coding-plan/provider.cjs` 当前实现依据的请求、响应形状和字段映射。它是示例 Provider 的实现说明，不代表第三方 API 的稳定官方契约；接口变化时应同步更新脚本、fixture 和本文档。

## 请求

| 项目 | 值 |
|------|----|
| Method | `GET` |
| URL | `https://open.bigmodel.cn/api/monitor/usage/quota/limit` |
| 鉴权 | `Authorization: Bearer <BIGMODEL_API_KEY>` |
| 必需环境变量 | `BIGMODEL_API_KEY` |
| Fixture 环境变量 | `QUOTABARWIN_BIGMODEL_FIXTURE` |

脚本在设置 `QUOTABARWIN_BIGMODEL_FIXTURE` 时跳过网络请求并读取本地 JSON，便于离线验证解析逻辑。

## 响应形状

当前解析器依赖下面这些字段：

```json
{
  "success": true,
  "code": 200,
  "msg": "ok",
  "data": {
    "level": "pro",
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
          {
            "modelCode": "search-prime",
            "usage": 32
          }
        ]
      }
    ]
  }
}
```

## 字段映射

| 原始字段 | 输出位置 | 说明 |
|----------|----------|------|
| `data.limits[]` | `windows[]` | 每个 limit 生成一个窗口。 |
| `type/unit/number` | `windows[].id` | 组合后把下划线换成短横线并转小写，例如 `tokens-limit-6-1`。 |
| `unit/number/type` | `windows[].label` | 生成可读标签，例如 `1 week - Tokens Limit`。 |
| `currentValue` | `windows[].used` | 当前用量，转为数字。 |
| `usage` | `windows[].limit` | 额度上限，转为数字。 |
| `percentage` | `windows[].usedPercent` | 已用百分比，转为数字。 |
| `100 - percentage` | `windows[].remainingPercent` | 剩余百分比，下限为 0。 |
| `nextResetTime` | `windows[].resetAt` | Unix epoch 毫秒转 ISO 时间。 |
| `type == "TOKENS_LIMIT"` | `windows[].unit = "tokens"` | 非 token 窗口的 `unit` 为 `null`。 |
| `data.level` | `metadata.level` | 套餐等级。 |
| `usageDetails[].modelCode/usage` | `metadata.usageDetails` | 按模型汇总的 Provider 专属用量。 |
| `code` | `metadata.rawCode` | 原始状态码。 |
| `msg` | `metadata.rawMsg` | 原始消息。 |

## 输出窗口

| 窗口 ID | Label 示例 | 来源 |
|---------|------------|------|
| `tokens-limit-3-5` | `5 hours - Tokens Limit` | `type = TOKENS_LIMIT, unit = 3, number = 5`。 |
| `tokens-limit-6-1` | `1 week - Tokens Limit` | `type = TOKENS_LIMIT, unit = 6, number = 1`。 |
| `time-limit-5-1` | `1 month - Time Limit` | `type = TIME_LIMIT, unit = 5, number = 1`。 |

脚本按上述顺序排序，未知 ID 排在后面。`unit` 的含义来自当前脚本中的映射：`3 = hour`、`5 = month`、`6 = week`。

## 错误与状态

- 缺少 `BIGMODEL_API_KEY` 时脚本退出并报错。
- HTTP 非 2xx 响应会抛出 `BigModel quota request failed with <status>`。
- 如果响应 JSON 中 `success === false`，脚本输出 `status: "error"`，并把 `msg` 放入 `error`。
- 其他可解析响应输出 `status: "ok"`。

## 本地参考

- 示例脚本：[`provider.cjs`](provider.cjs)
- Manifest：[`provider.json`](provider.json)
- 原始响应 fixture：[`../../../fixtures/provider_outputs/bigmodel_quota_limit.json`](../../../fixtures/provider_outputs/bigmodel_quota_limit.json)
- 命令 fixture 包装：[`../../../fixtures/commands/bigmodel_quota_fixture.js`](../../../fixtures/commands/bigmodel_quota_fixture.js)
