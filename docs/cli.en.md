# Agent / CLI Guide

Simplified Chinese version: [`cli.md`](cli.md).

`QuotaBarWin.Cli.exe` is QuotaBarWin's command-line entry point for agents, CI, and scripts. It reuses the desktop app's configuration, secret placeholders, installed remote Providers, and Provider cache. It does not open a window or tray, or start a second GUI instance.

In the portable release, `QuotaBarWin.Cli.exe` is next to `QuotaBarWin.exe`. Both recognize the adjacent `quotabarwin.portable` marker and therefore share portable configuration, Provider cache, and snapshots. Without that marker, both use `%APPDATA%\QuotaBarWin`.

## Commands

```powershell
# List fresh data for every enabled Provider; refresh is the default
.\QuotaBarWin.Cli.exe get

# Refresh and read one window from one installed instance
.\QuotaBarWin.Cli.exe get --provider codex-usage --window 5h

# Do not use the network; read the last persisted snapshot only
.\QuotaBarWin.Cli.exe get --provider codex-usage --cached

# Let an agent decide whether it can start a quota-intensive step
.\QuotaBarWin.Cli.exe check --provider codex-usage --window 5h --min-remaining-percent 20
```

`get` and `check` default to `--refresh`. This runs the installed Provider scripts and their network requests according to their configured settings, and updates the snapshot visible to the desktop app. `--cached` reads only `last_snapshot.quotaBarWin.json`; it fails if no snapshot exists. Do not use an unconstrained-age cached result for high-risk or quota-intensive decisions.

Use `--config C:\path\config.quotaBarWin.json` to select an explicit local config, for example in an isolated test environment. Do not pass tokens, cookies, API keys, or proxy credentials on the command line. Continue to use the existing `${secret:...}`, `${env:...}`, and `${file:...}` configuration mechanisms.

## Output protocol

Except for `--help` and `--version`, every command writes one JSON object to stdout. The protocol version is `schemaVersion: 1`. Output includes only generic Provider and quota-window fields; `metadata`, Provider diagnostics, and stderr are excluded so Provider-private data is not sent into agent logs.

`get` has this shape:

```json
{
  "schemaVersion": 1,
  "dataSource": "refresh",
  "refreshedAt": "2026-07-10T00:00:00Z",
  "providers": [
    {
      "id": "codex-usage",
      "name": "Codex",
      "status": "ok",
      "updatedAt": "2026-07-10T00:00:00Z",
      "windows": [
        {
          "id": "5h",
          "remainingPercent": 42,
          "resetAt": "2026-07-10T05:00:00Z",
          "confidence": "high"
        }
      ]
    }
  ]
}
```

`dataSource` is either `refresh` or `cache`. Each Provider retains its current status; an `error` or `stale` status is not safe input for automatically continuing work. Window fields follow the public `QuotaWindow` contract. Whether a Provider supplies absolute `remaining`, `limit`, percentages, or `resetAt` depends on that Provider.

## `check` exit codes and decisions

`check` requires `--provider`, `--window`, and `--min-remaining-percent` (0–100). Its JSON `decision` means:

| Exit code | decision | Meaning |
|---:|---|---|
| 0 | `continue` | `remainingPercent` is at or above the threshold. |
| 10 | `defer` | Remaining percentage is below the threshold; defer quota-intensive work or switch tasks. |
| 11 | `unknown` | The Provider is `error` / `stale`, or lacks a valid `remainingPercent`; do not treat this as available quota. |
| 20 | — | Configuration, refresh, cache, or Provider/window lookup failed. |
| 64 | — | Invalid command arguments. |

For command-level errors, stdout is still JSON:

```json
{
  "schemaVersion": 1,
  "error": {
    "code": "check_failed",
    "message": "Quota window 5h was not found"
  }
}
```

## Agent orchestration example

In PowerShell, an agent wrapper can branch on the exit code alone:

```powershell
.\QuotaBarWin.Cli.exe check --provider codex-usage --window 5h --min-remaining-percent 20
switch ($LASTEXITCODE) {
  0  { "Continue the quota-intensive step" }
  10 { "Do lower-cost work; recheck around resetAt" }
  11 { "Quota is unknown; back off briefly and refresh again, without assuming availability" }
  default { throw "QuotaBarWin CLI check failed: $LASTEXITCODE" }
}
```

For long-running work, check at phase boundaries or before expensive operations, not before every small action. QuotaBarWin uses a cross-process lock between GUI and CLI refreshes to prevent them from running Providers at the same time, though a Provider can still have its own rate limits.

`resetAt` is a Provider-reported reset or suggested recheck time. It can represent a rolling window, a relative retry time, or a server-side constraint; it is not a promise that quota will be completely restored at that time. Run `check --refresh` again around `resetAt` before resuming quota-intensive work.
