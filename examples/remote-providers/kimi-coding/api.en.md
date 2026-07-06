# Kimi Coding API Notes

Default documentation is Simplified Chinese: [`api.md`](api.md).

This file documents the request, response shape, and field mapping used by `kimi-coding/provider.cjs`. It describes the example provider implementation, not a stable official third-party API contract. If the API changes, update the script, fixtures, and this document together.

## Request

| Item | Value |
|------|-------|
| Method | `GET` |
| URL | `https://api.kimi.com/coding/v1/usages` |
| Auth | `Authorization: Bearer <KIMI_API_KEY>` |
| Required env var | `KIMI_API_KEY` |
| Fixture env var | `QUOTABARWIN_KIMI_FIXTURE` |

When `QUOTABARWIN_KIMI_FIXTURE` is set, the script skips the network request and reads local JSON instead.

## Response Shape

The current parser depends on these fields:

```json
{
  "limits": [
    {
      "window": {
        "duration": 300,
        "timeUnit": "TIME_UNIT_MINUTE"
      },
      "detail": {
        "used": "10",
        "remaining": "90",
        "limit": "100",
        "resetTime": "2026-06-07T16:35:14.207781Z"
      }
    }
  ],
  "usage": {
    "used": "15",
    "remaining": "85",
    "limit": "100",
    "resetTime": "2026-06-12T02:35:14.207781Z"
  },
  "totalQuota": {
    "limit": "100",
    "remaining": "99"
  },
  "user": {
    "region": "REGION_CN",
    "membership": {
      "level": "LEVEL_INTERMEDIATE"
    }
  },
  "parallel": {
    "limit": "20"
  },
  "authentication": {
    "method": "METHOD_API_KEY",
    "scope": "FEATURE_CODING"
  },
  "subType": "TYPE_PURCHASE"
}
```

## Field Mapping

| Raw field | Output | Notes |
|-----------|--------|-------|
| `limits[].window.duration == 300` and `timeUnit == "TIME_UNIT_MINUTE"` | `windows[].id = "300-minute"` | The 300-minute window is shown as `5h`. |
| `limits[].detail.used` | `windows[].used` | Converted to a number; empty or invalid values become `null`. |
| `limits[].detail.remaining` | `windows[].remaining` | Converted to a number and used for remaining percentage. |
| `limits[].detail.limit` | `windows[].limit` | Converted to a number and used with `used` or `remaining` for percentages. |
| `limits[].detail.resetTime` | `windows[].resetAt` | Used as the ISO timestamp. |
| `usage` | `windows[].id = "usage"` | Mapped to `Weekly limit`. |
| `totalQuota.limit - totalQuota.remaining` | `windows[].id = "total-quota"` `used` | Total quota emits absolute usage only. |
| `user.region` | `metadata.region` | Account region. |
| `user.membership.level` | `metadata.membershipLevel` | Membership level. |
| `authentication.method` | `metadata.authMethod` | Auth method. |
| `authentication.scope` | `metadata.authScope` | Auth scope. |
| `subType` | `metadata.subType` | Plan or quota source type. |
| `parallel.limit` | `metadata.parallelLimit` | Parallel limit, converted to a number. |

If the 300-minute window has no `used`, `remaining`, or `limit`, the script treats unknown usage as fully available: `usedPercent = 0`, `remainingPercent = 100`, and `confidence = "estimated"`.

## Output Windows

| Window ID | Label | Source |
|-----------|-------|--------|
| `300-minute` | `5h` | The 300-minute entry in `limits[].detail`. |
| `usage` | `Weekly limit` | Top-level `usage`. |
| `total-quota` | `Total quota` | Top-level `totalQuota`. |

## Errors And Status

- Missing `KIMI_API_KEY` exits with an error.
- Non-2xx responses throw `Kimi usage request failed with <status>`.
- When the network request succeeds and JSON parses, the provider emits `status: "ok"`; missing optional windows only reduce `windows[]`.

## Local References

- Script: [`provider.cjs`](provider.cjs)
- Manifest: [`provider.json`](provider.json)
- Raw response fixture: [`../../../docs/specs/fixtures/provider_outputs/kimi_coding_usage.json`](../../../docs/specs/fixtures/provider_outputs/kimi_coding_usage.json)
- Command fixture wrapper: [`../../../fixtures/commands/kimi_usage_fixture.js`](../../../fixtures/commands/kimi_usage_fixture.js)
