# BigModel Coding Plan API Notes

Default documentation is Simplified Chinese: [`api.md`](api.md).

This file documents the request, response shape, and field mapping used by `bigmodel-coding-plan/provider.cjs`. It describes the example provider implementation, not a stable official third-party API contract. If the API changes, update the script, fixtures, and this document together.

## Request

| Item | Value |
|------|-------|
| Method | `GET` |
| URL | `https://open.bigmodel.cn/api/monitor/usage/quota/limit` |
| Auth | `Authorization: Bearer <BIGMODEL_API_KEY>` |
| Required env var | `BIGMODEL_API_KEY` |
| Fixture env var | `QUOTABARWIN_BIGMODEL_FIXTURE` |

When `QUOTABARWIN_BIGMODEL_FIXTURE` is set, the script skips the network request and reads local JSON instead.

## Response Shape

The current parser depends on these fields:

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

## Field Mapping

| Raw field | Output | Notes |
|-----------|--------|-------|
| `data.limits[]` | `windows[]` | Each limit becomes one window. |
| `type/unit/number` | `windows[].id` | Joined, underscores replaced with dashes, lowercased, for example `tokens-limit-6-1`. |
| `unit/number/type` | `windows[].label` | Human-readable label, for example `1 week - Tokens Limit`. |
| `currentValue` | `windows[].used` | Current usage, converted to a number. |
| `usage` | `windows[].limit` | Quota limit, converted to a number. |
| `percentage` | `windows[].usedPercent` | Used percentage, converted to a number. |
| `100 - percentage` | `windows[].remainingPercent` | Remaining percentage, floored at 0. |
| `nextResetTime` | `windows[].resetAt` | Unix epoch milliseconds converted to ISO time. |
| `type == "TOKENS_LIMIT"` | `windows[].unit = "tokens"` | Other window units are `null`. |
| `data.level` | `metadata.level` | Plan level. |
| `usageDetails[].modelCode/usage` | `metadata.usageDetails` | Provider-specific usage by model. |
| `code` | `metadata.rawCode` | Raw status code. |
| `msg` | `metadata.rawMsg` | Raw message. |

## Output Windows

| Window ID | Example label | Source |
|-----------|---------------|--------|
| `tokens-limit-3-5` | `5 hours - Tokens Limit` | `type = TOKENS_LIMIT, unit = 3, number = 5`. |
| `tokens-limit-6-1` | `1 week - Tokens Limit` | `type = TOKENS_LIMIT, unit = 6, number = 1`. |
| `time-limit-5-1` | `1 month - Time Limit` | `type = TIME_LIMIT, unit = 5, number = 1`. |

The script sorts windows in that order and places unknown IDs last. Unit labels come from the script mapping: `3 = hour`, `5 = month`, and `6 = week`.

## Errors And Status

- Missing `BIGMODEL_API_KEY` exits with an error.
- Non-2xx responses throw `BigModel quota request failed with <status>`.
- If the parsed JSON has `success === false`, the provider emits `status: "error"` and copies `msg` to `error`.
- Other parseable responses emit `status: "ok"`.

## Local References

- Script: [`provider.cjs`](provider.cjs)
- Manifest: [`provider.json`](provider.json)
- Raw response fixture: [`../../../docs/specs/fixtures/provider_outputs/bigmodel_quota_limit.json`](../../../docs/specs/fixtures/provider_outputs/bigmodel_quota_limit.json)
- Command fixture wrapper: [`../../../fixtures/commands/bigmodel_quota_fixture.js`](../../../fixtures/commands/bigmodel_quota_fixture.js)
