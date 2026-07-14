# Time Flies Provider Notes

Default documentation is Simplified Chinese: [`api.md`](api.md).

`time-flies/provider.cjs` does not access the network, files, or account credentials. It uses only the local timezone and current time of the machine that runs it to emit Time Flies windows in minutes.

## Runtime Requirements

| Item | Value |
|------|-------|
| Runtime | `node` |
| Required environment variables | None |
| Network / credential / file permissions | None |
| Output protocol | `provider-snapshot-v1` |

## Calculation Rules

- All calendar boundaries use the machine's **local timezone**. `resetAt` is the ISO representation of that local boundary.
- Every window reports the number of **remaining minutes** to its next boundary, rounded up. For example, Wednesday afternoon reports the precise minute count to the following Monday midnight.
- The Monday-start window resets at the next Monday midnight; the Sunday-start window resets at the next Sunday midnight.
- Month and year limits use the actual calendar length, including short months and leap years.

## Output Windows

| Window ID | Default label | Unit | Limit | Reset time |
|-----------|---------------|------|-------|------------|
| `today` | Today remaining | `minutes` | 1440 | Next local midnight |
| `week-monday` | Week remaining (Monday start) | `minutes` | 10080 | Next Monday midnight |
| `week-sunday` | Week remaining (Sunday start) | `minutes` | 10080 | Next Sunday midnight |
| `month` | Month remaining | `minutes` | Number of minutes in the current month | First day of next month at midnight |
| `year` | Year remaining | `minutes` | Number of minutes in the current year | January 1 of next year at midnight |

Each window supplies `remaining`, `used`, `limit`, `remainingPercent`, `usedPercent`, and `resetAt`, so QuotaBarWin can use its normal remaining/used display modes, progress bar, and tray views. `metadata.timezone` records the detected runtime timezone and `metadata.precision` is always `minutes`.

## Local References

- Script: [`provider.cjs`](provider.cjs)
- Manifest: [`provider.json`](provider.json)
- Output contract: [`../../../docs/remote-provider-guide.en.md`](../../../docs/remote-provider-guide.en.md)
