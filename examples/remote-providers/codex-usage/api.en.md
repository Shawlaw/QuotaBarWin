# Codex Usage API Notes

Default documentation is Simplified Chinese: [`api.md`](api.md).

This file documents the request, response shape, and field mapping used by `codex-usage/provider.cjs`. It describes the example provider implementation, not a stable official third-party API contract. If the API changes, update the script and this document together.

## Request

| Item | Value |
|------|-------|
| Method | `GET` |
| URL | `https://chatgpt.com/backend-api/wham/usage` |
| Auth | `Authorization: Bearer <access token>` |
| Fixed header | `User-Agent: QuotaBarWin/0.0` |
| Optional header | `ChatGPT-Account-Id: <account id>` |
| Timeout | `30000` ms |

Token and account id sources, in priority order:

1. `CODEX_ACCESS_TOKEN` / `CODEX_ACCOUNT_ID`
2. The JSON file pointed to by `CODEX_AUTH_FILE`
3. `~/.codex/auth.json`

The local Codex auth file currently reads `tokens.access_token` and `tokens.account_id`. The script also supports runtime proxies, preferring `QBWIN_PROXY_URL`, then `HTTPS_PROXY`, `HTTP_PROXY`, and `ALL_PROXY`. In QuotaBarWin, set `QBWIN_PROXY_URL` explicitly through that Provider's environment variables; when absent, the host uses the project-wide proxy as a fallback. An installation-source proxy is only used to download registries, manifests, and scripts, and is never injected into the Provider runtime. Supported proxy protocols are `socks5:`, `socks5h:`, `http:`, and `https:`.

## Response Shape

The current parser depends on these fields:

```json
{
  "plan_type": "plus",
  "credits": {
    "granted": 100,
    "used": 12
  },
  "rate_limit": {
    "primary_window": {
      "used_percent": 32,
      "reset_after_seconds": 7200,
      "limit_window_seconds": 18000
    },
    "secondary_window": {
      "used_percent": 18,
      "reset_at": 1781913600,
      "limit_window_seconds": 604800
    }
  }
}
```

`reset_at` is Unix epoch seconds. If `reset_at` is absent but `reset_after_seconds` is present, the script adds that relative value to the current time and emits ISO time.

## Field Mapping

| Raw field | Output | Notes |
|-----------|--------|-------|
| `rate_limit.*_window.limit_window_seconds` | `windows[].id` | Prefer `18000` seconds for `5h` and `604800` seconds for `weekly`, without trusting primary / secondary position. |
| `rate_limit.primary_window` / `secondary_window` | `windows[]` | When `limit_window_seconds` is missing or unknown, retain the legacy mapping: primary is `5h`, secondary is `weekly`. |
| `used_percent` | `windows[].usedPercent` | Converted to a number. |
| `100 - used_percent` | `windows[].remainingPercent` | Remaining percentage, floored at 0. |
| `reset_at` | `windows[].resetAt` | Unix epoch seconds converted to ISO time. |
| `reset_after_seconds` | `windows[].resetAt` | Current time plus relative seconds, converted to ISO time. |
| `plan_type` | `metadata.planType` | Account plan type. |
| `credits` | `metadata.credits` | Preserved as returned. |

The Codex usage API currently reports percentages rather than absolute counters, so the script keeps `used` and `limit` as `null` and sets `unit` to `percent`.

## Output Windows

| Window ID | Label | Source |
|-----------|-------|--------|
| `5h` | `5h` | Window with `limit_window_seconds = 18000`; falls back to `primary_window` when absent. |
| `weekly` | `Weekly limit` | Window with `limit_window_seconds = 604800`; falls back to `secondary_window` when absent. |

## Errors And Status

- Missing access token exits with guidance to set `CODEX_ACCESS_TOKEN` or sign in with Codex so the local auth file exists.
- Non-`200` responses throw `Codex usage API returned <status>`.
- Invalid JSON throws a parse error.
- At least one window emits `status: "ok"`; no usable windows emits `status: "warning"`.

## Local References

- Script: [`provider.cjs`](provider.cjs)
- Manifest: [`provider.json`](provider.json)
- Output contract: [`../../../docs/remote-provider-guide.md`](../../../docs/remote-provider-guide.md)
