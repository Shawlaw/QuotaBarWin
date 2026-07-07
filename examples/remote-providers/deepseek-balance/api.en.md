# DeepSeek Balance API Notes

Default documentation is Simplified Chinese: [`api.md`](api.md).

This file documents the request, response shape, and field mapping used by `deepseek-balance/provider.cjs`. It describes the example provider implementation, not a stable official third-party API contract. If the API changes, update the script, fixtures, and this document together.

## Request

| Item | Value |
|------|-------|
| Method | `GET` |
| URL | `https://api.deepseek.com/user/balance` |
| Auth | `Authorization: Bearer <DEEPSEEK_API_KEY>` |
| Fixed header | `Accept: application/json` |
| Required env var | `DEEPSEEK_API_KEY` |
| Fixture env var | `QUOTABARWIN_DEEPSEEK_FIXTURE` |

Optional local display configuration:

| Env var | Notes |
|---------|-------|
| `DEEPSEEK_BALANCE_CURRENCY` | Show only this currency; if the response does not include it, the script errors. |
| `DEEPSEEK_BALANCE_REFERENCE_TOTAL` | Reference total used to compute `used` and progress bars. |
| `DEEPSEEK_BALANCE_REFERENCE_TOTAL_<CURRENCY>` | Per-currency reference total override, for example `DEEPSEEK_BALANCE_REFERENCE_TOTAL_CNY`. |
| `DEEPSEEK_BALANCE_WARNING` | Low-balance warning threshold. |
| `DEEPSEEK_BALANCE_WARNING_<CURRENCY>` | Per-currency warning override, for example `DEEPSEEK_BALANCE_WARNING_CNY`. |

When `QUOTABARWIN_DEEPSEEK_FIXTURE` is set, the script skips the network request and reads local JSON instead.

## Response Shape

The current parser depends on these fields:

```json
{
  "is_available": true,
  "balance_infos": [
    {
      "currency": "CNY",
      "total_balance": "110.00",
      "granted_balance": "10.00",
      "topped_up_balance": "100.00"
    }
  ]
}
```

## Field Mapping

| Raw field or local config | Output | Notes |
|---------------------------|--------|-------|
| `balance_infos[]` | `windows[]` | Each currency becomes one balance window. |
| `currency` | `windows[].id` | Produces stable IDs such as `balance-cny`. |
| `currency` | `windows[].label` | Produces labels such as `CNY balance`. |
| `total_balance` | `windows[].remaining` | Current balance, converted to a number. |
| `currency` | `windows[].unit` | Balance unit. |
| `DEEPSEEK_BALANCE_REFERENCE_TOTAL[_<CURRENCY>]` | `windows[].limit` | Reference total, converted to a number. |
| `referenceTotal - total_balance` | `windows[].used` | Computed only when both reference total and balance are available. |
| `DEEPSEEK_BALANCE_WARNING[_<CURRENCY>]` | `windows[].warningRemaining` | Low-balance threshold, converted to a number. |
| `total_balance` | `windows[].resetText` | Shown as `Balance <amount> <currency>`. |
| `is_available` | `metadata.isAvailable` | Account availability. |
| `DEEPSEEK_BALANCE_CURRENCY` | `metadata.currency` | Requested currency filter. |
| `balance_infos[]` | `metadata.balances` | Preserves `totalBalance`, `grantedBalance`, and `toppedUpBalance` by currency. |

DeepSeek returns balance, not a recurring quota. Without a reference total, the window still shows `remaining`, but `used` and `limit` stay `null`.

## Output Windows

| Window ID | Example label | Source |
|-----------|---------------|--------|
| `balance-cny` | `CNY balance` | `currency = "CNY"` in `balance_infos[]`. |

## Errors And Status

- Missing `DEEPSEEK_API_KEY` exits with an error.
- Non-2xx responses throw `DeepSeek balance request failed with <status>`.
- If `DEEPSEEK_BALANCE_CURRENCY` is set and the response has no matching currency, the script errors.
- `is_available === false` emits `status: "warning"`.
- Any window at or below the local warning threshold emits `status: "warning"` with diagnostics.
- Other parseable responses emit `status: "ok"`.

## Local References

- Script: [`provider.cjs`](provider.cjs)
- Manifest: [`provider.json`](provider.json)
- Raw response fixture: [`../../../fixtures/provider_outputs/deepseek_balance.json`](../../../fixtures/provider_outputs/deepseek_balance.json)
