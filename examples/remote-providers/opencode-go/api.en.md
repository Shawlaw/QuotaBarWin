# OpenCode Go Usage API notes

Chinese version: [`api.md`](api.md).

This document records the request, response shapes, and field mapping currently used by `opencode-go/provider.js` (`builtin-js`). It describes this example provider, not a stable public contract for a third-party API; update the script and this document together when the endpoint changes.

## Request

| Item | Value |
|------|-------|
| Method | `GET` |
| URL | `https://opencode.ai/zen/go/v1/usage` |
| Auth | `Authorization: Bearer <OPENCODE_GO_API_KEY>` |
| Fixed headers | `Accept: application/json`, `User-Agent: QuotaBarWin/1.7` |
| Timeout | The provider instance's `timeoutSeconds` (30 seconds by default) |

The key comes only from provider configuration: paste the OpenCode Go API key into the post-install **Save and test** form, and the app stores it as an instance-isolated managed secret injected as `OPENCODE_GO_API_KEY`. This example supports API keys only; it does not read the local opencode login or SQLite database. See the `codex-usage` local auth-file pattern if you need to reuse a local CLI login.

The host handles proxying: the provider Proxy URL wins, then the global proxy. Scripts never see proxy credentials. The install-source proxy is only used to download the registry, manifest, and script. `socks5:`, `socks5h:`, `http:`, and `https:` proxies are supported.

## Response shapes

The primary shape is a `usage` object with three windows:

```json
{
  "usage": {
    "mine": true,
    "useBalance": false,
    "rolling": { "status": "ok", "percent": 12, "resetsAt": "2026-11-04T13:20:00.000Z" },
    "weekly": { "status": "ok", "percent": 34, "resetsAt": "2026-11-09T00:00:00.000Z" },
    "monthly": { "status": "ok", "percent": 56, "resetsAt": "2026-12-03T08:30:00.000Z" }
  }
}
```

`percent` is the **used** percentage in 0–100 units: `1` means 1% used, and `0.5` means 0.5% used. It is not a 0–1 fraction.

The script also accepts two other shapes:

- Console page hydration names: `usage.rollingUsage` / `weeklyUsage` / `monthlyUsage`, with `usagePercent` and the relative-seconds reset field `resetInSec`.
- The legacy proposed `usage.windows[]` array, where each entry matches `rolling`, `weekly`, or `monthly` through `name` (or `id`) and provides `percent` or `usagePercent`.

## Field mapping

| Raw field | Output | Notes |
|-----------|--------|-------|
| `usage.rolling` / `rollingUsage` | The `5h` window in `windows[]` | 5-hour rolling window. |
| `usage.weekly` / `weeklyUsage` | The `weekly` window in `windows[]` | Weekly window; omitted when missing. |
| `usage.monthly` / `monthlyUsage` | The `monthly` window in `windows[]` | Monthly window; omitted when missing. |
| `percent` / `usagePercent` | `windows[].usedPercent` | Clamped to 0–100. |
| `100 - percent` | `windows[].remainingPercent` | Remaining percentage, floored at 0. |
| `resetsAt` | `windows[].resetAt` | ISO timestamp. |
| `resetInSec` / `resetAfterSeconds` | `windows[].resetAt` | Current time plus relative seconds, converted to ISO. |
| `useBalance` (top-level or inside `usage`) | `metadata.useBalance` | Optional boolean preserved as-is. |
| `plan` / `billing.subscriptionPlan` | `metadata.plan` | Optional plan name. |

The endpoint only reports percentages, not balances or absolute allowances, so `used` and `limit` stay `null` and `unit` is `percent`. OpenCode Go has no separate balance amount in this response; use the Billing page in the OpenCode Console for spend.

## Output windows

| Window ID | Label | Source |
|-----------|-------|--------|
| `5h` | `5h` | `usage.rolling` (API) or `rollingUsage` (console shape). |
| `weekly` | `Weekly limit` | `usage.weekly` or `weeklyUsage`. |
| `monthly` | `Monthly limit` | `usage.monthly` or `monthlyUsage`. |

Windows are emitted in 5h, weekly, monthly order; a window missing from the response is never created as an empty window. `defaultConfig.visibleWindowIds` shows all three initially, and users can still adjust visible windows and labels in Settings.

## Errors and status

- Without `OPENCODE_GO_API_KEY`, the script exits with a message to create an API key in the OpenCode Console.
- HTTP `401` reports that the key was rejected; HTTP `403` reports that the key may lack Usage permission or that the account has no Go subscription.
- Other non-2xx statuses raise `OpenCode Go usage API returned <status>` with at most the first 160 characters of the body.
- A body that cannot be parsed as JSON raises a parse error.
- A response without a `usage` object raises an explicit error saying the key must belong to an account with a Go subscription.
- With at least one parsed window the provider returns `status: "ok"`; when `usage` exists but no recognized window is present it returns `status: "warning"` with an empty `windows[]`.

## Local references

- Raw response fixture: [`../../../fixtures/provider_outputs/opencode_go_usage.json`](../../../fixtures/provider_outputs/opencode_go_usage.json)
- Example script: [`provider.js`](provider.js)
- Manifest: [`provider.json`](provider.json)
- Output protocol: [`../../../docs/remote-provider-guide.en.md`](../../../docs/remote-provider-guide.en.md)
