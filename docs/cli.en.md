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

# Validate an installed Provider's config, cached manifest, source checksum, and runtime
.\QuotaBarWin.Cli.exe validate --provider codex-usage

# Validate local manifest and source files before installation
.\QuotaBarWin.Cli.exe validate --manifest .\provider.json --source .\provider.cjs
```

`get` and `check` default to `--refresh`. This runs the installed Provider scripts and their network requests according to their configured settings, and updates the snapshot visible to the desktop app. `--cached` reads only `last_snapshot.quotaBarWin.json`; it fails if no snapshot exists. Do not use an unconstrained-age cached result for high-risk or quota-intensive decisions.

With `--provider ID --refresh`, the CLI refreshes only that instance. It refreshes every enabled Provider only when `--provider` is omitted.

Use `--config C:\path\config.quotaBarWin.json` to select an explicit local config, for example in an isolated test environment. Do not pass tokens, cookies, API keys, or proxy credentials on the command line. Continue to use the existing `${secret:...}`, `${env:...}`, and `${file:...}` configuration mechanisms.

## Provider validation

`validate` is for Provider authors, CI, and troubleshooting. By default it does not run the source script or make Provider API requests; it runs only when `--run` is explicitly supplied.

- `validate --provider ID` reads the local config and cached Provider. It checks `providerDir`, `timeoutSeconds`, consistency between config and manifest runtime, configured required env-var names, the manifest, source file, checksum, and runtime availability.
- `validate --manifest PATH` checks a local manifest. It automatically locates the source when `entry` is a relative local path. When entry is a URL, pass `--source PATH`.

To validate the live output protocol too, use `validate --provider ID --run`. It executes the cached script using that Provider's configured runtime, secrets, and proxy, then verifies that stdout is accepted by the current `provider-snapshot-v1` parser. This can access the network and account API, so it is opt-in. The report's `scriptRun` is `notRun`, `passed`, or `failed`.

It requires the public `provider-snapshot-v1` output protocol, `node`, `python`, `pwsh`, `bash`, or an absolute runtime path, and a non-empty `displayName`. `checksums.source` remains optional in the public contract: its absence causes a warning but does not alone fail validation. The report never emits configured environment-variable values, secrets, or Provider stderr.

Both valid and invalid reports are JSON on stdout. `valid: false` exits `30`; command-level errors such as an unreadable manifest exit `20`. For example:

```json
{
  "schemaVersion": 1,
  "target": { "kind": "installed", "providerId": "codex-usage" },
  "valid": true,
  "errors": [],
  "warnings": [],
  "runtime": { "declared": "node", "resolved": "C:\\Program Files\\nodejs\\node.exe" },
  "checksum": { "expected": "sha256:…", "actual": "sha256:…", "matches": true }
}
```

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
| 30 | — | `validate` found a nonconforming Provider config, manifest, source, checksum, or runtime. |
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
