# QuotaBarWin Remote Provider Guide

Default documentation is Simplified Chinese:
[`remote-provider-guide.md`](remote-provider-guide.md).

Remote providers let you install quota providers from a hosted manifest + script, without bundling them into the app. This is useful for third-party providers, rapid iteration, or sharing provider configurations across machines.

## How it works

1. You provide a registry URL or local file path (`registry.json`) that lists one or more providers. Both `https://` and `file://` URLs, as well as plain local paths like `C:\Providers\registry.json`, are supported.
2. QuotaBarWin reads the registry, fetches each referenced provider manifest, verifies optional checksums, and downloads the source scripts.
3. Each script is cached locally and executed with its declared runtime (for example `node`, `python`, or an absolute path).
4. On every refresh QuotaBarWin runs the cached scripts and parses the output into quota windows.

## Manifest format (`provider.json`)

```json
{
  "schemaVersion": 1,
  "id": "kimi-coding",
  "displayName": "Kimi Coding Usage",
  "description": "Kimi coding quota usage via remote provider script",
  "runtime": "node",
  "entry": "provider.cjs",
  "requiredEnvVars": ["KIMI_API_KEY"],
  "output": "provider-snapshot-v1",
  "permissions": ["env:KIMI_API_KEY"],
  "checksums": {
    "source": "sha256:<hex>"
  }
}
```

Field descriptions:

| Field | Required | Description |
|-------|----------|-------------|
| `schemaVersion` | yes | Must be `1`. |
| `id` | yes | Unique provider id. Must not conflict with an existing provider in your config. |
| `displayName` | yes | Human-readable name shown in the UI. |
| `description` | no | Short description. |
| `runtime` | yes | Runtime used to execute `entry`. Common values: `node`, `python`, `pwsh`, `bash`. Can also be an absolute path like `C:\Tools\node\node.exe`. |
| `entry` | yes | Source file name. Can be a relative path (resolved against the manifest URL/directory), an absolute HTTPS URL, a `file://` URL, or a local file path. |
| `requiredEnvVars` | no | Environment variables that the script needs. On refresh, QuotaBarWin resolves each name from provider `envVars`, then `${secret:NAME}`. |
| `output` | yes | Output contract. Only `provider-snapshot-v1` is supported for remote providers at the moment. |
| `permissions` | no | Declared capabilities (currently informational). Use `env:<NAME>` to document required env vars. |
| `checksums.source` | no | SHA-256 checksum of the source file. Required if you want `autoUpdate` to work. Format: `sha256:<hex>`. |

## Provider registry (`registry.json`)

A registry lets you install multiple providers with one URL. It is useful for distributing a curated set of providers.

```json
{
  "schemaVersion": 1,
  "providers": [
    {
      "id": "kimi-coding",
      "providerUrl": "kimi-coding/provider.json",
      "checksum": "sha256:<manifest-sha256>"
    }
  ]
}
```

Field descriptions:

| Field | Required | Description |
|-------|----------|-------------|
| `schemaVersion` | yes | Must be `1`. |
| `providers` | yes | Array of provider entries. |
| `providers[].id` | yes | Provider id. Must match the id declared in the referenced manifest. |
| `providers[].providerUrl` | yes | URL or local path to the provider's `provider.json`. Relative paths are resolved against the registry URL/path. |
| `providers[].checksum` | no | SHA-256 checksum of the referenced manifest text. If provided, QuotaBarWin verifies the manifest before installing. |

## Source script output contract

When `output` is `provider-snapshot-v1`, the script must print a single JSON object to stdout. `id`, `name`, and `source` are optional and default to the values from the manifest/config.

```json
{
  "status": "ok",
  "updatedAt": "2026-06-12T10:00:00.000Z",
  "windows": [
    {
      "id": "weekly",
      "label": "Weekly limit",
      "remaining": 88,
      "used": 12,
      "limit": 100,
      "unit": "requests",
      "usedPercent": 12,
      "remainingPercent": 88,
      "warningRemaining": 20,
      "resetAt": "2026-06-19T10:00:00.000Z",
      "resetText": null,
      "confidence": "exact"
    }
  ],
  "metadata": {}
}
```

Window fields:

| Field | Required | Description |
|-------|----------|-------------|
| `id` | yes | Stable window identifier used by user configuration. |
| `label` | yes | Display label. Can be friendly or localized, but must not be the only stable identity. |
| `used` | no | Used amount. |
| `remaining` | no | Remaining amount. Useful for balance providers that report the current balance directly. |
| `limit` | no | Total limit. |
| `unit` | no | Unit string, e.g. `requests`, `tokens`, `percent`. |
| `usedPercent` | no | 0-100. |
| `remainingPercent` | no | 0-100. |
| `warningRemaining` | no | Absolute remaining amount that should put the window in warning state. |
| `resetAt` | no | ISO 8601 timestamp. |
| `resetText` | no | Human-readable reset text. |
| `confidence` | no | `exact`, `estimated`, or `unknown`. |

### Stable window IDs and user customization

Treat every `windows[].id` value as part of your provider's compatibility contract. Users can customize per-window display order, visibility, and names by referring to these IDs in local provider config, so changing an ID can silently break their preferences.

Example installed provider config:

```json
{
  "kind": "remote",
  "id": "kimi-coding",
  "visibleWindowIds": ["300-minute", "usage", "total-quota"],
  "windowLabelOverrides": {
    "300-minute": "5h",
    "usage": "Weekly"
  },
  "envVars": {
    "KIMI_API_KEY": "${secret:KIMI_API_KEY}"
  }
}
```

- `visibleWindowIds` controls which windows are displayed. When it is set, QuotaBarWin displays only those windows and uses the configured order.
- `windowLabelOverrides` controls display names. Overrides are matched by `window.id` first, so stable IDs let users keep custom names even if provider labels change. Legacy label matching may work for existing configs, but provider authors should document and preserve IDs.
- `label` should be friendly text for the UI and may change for clarity or localization. Do not derive `id` from translated labels, marketing copy, or other wording that might change. Prefer semantic provider API keys such as `weekly`, `300-minute`, `tokens-limit-6-1`, or `total-quota`.

## Parsing raw API responses

Remote providers should keep provider-specific parsing inside the source script. The app only needs the normalized `provider-snapshot-v1` JSON printed to stdout.

Recommended parsing flow:

1. Fetch or read the provider's raw API response.
2. Select the quota records that represent user-visible windows.
3. Convert provider-specific field names into stable window fields.
4. Put useful extra provider fields in `metadata`, not in `windows`.
5. Print exactly one JSON object to stdout; write diagnostics to stderr.

Example raw BigModel response:

```json
{
  "success": true,
  "code": 200,
  "msg": "success",
  "data": {
    "level": "pro",
    "limits": [
      {
        "type": "TOKENS_LIMIT",
        "unit": 6,
        "number": 1,
        "currentValue": 12345,
        "usage": 100000,
        "percentage": 12.35,
        "nextResetTime": 1781654400000,
        "usageDetails": [
          { "modelCode": "glm-4.5", "usage": 1000 }
        ]
      }
    ]
  }
}
```

Mapping into `provider-snapshot-v1`:

| Raw field | Snapshot field | Notes |
|-----------|----------------|-------|
| `data.limits[]` | `windows[]` | One raw limit becomes one quota window. |
| `type`, `unit`, `number` | `id`, `label` | Build a stable id and a readable label. |
| `currentValue` | `used` | Normalize strings/numbers to numbers when possible. |
| `usage` | `limit` | Leave as `null` if the API omits it. |
| `percentage` | `usedPercent` | Clamp or validate into the 0-100 range if the API is not trusted. |
| `100 - percentage` | `remainingPercent` | Use `null` when `percentage` is missing. |
| `nextResetTime` | `resetAt` | Convert epoch milliseconds to ISO 8601. |
| `level`, `usageDetails`, raw status fields | `metadata` | Preserve useful details without changing the window contract. |

Example raw Kimi response:

```json
{
  "limits": [
    {
      "window": { "duration": 300, "timeUnit": "TIME_UNIT_MINUTE" },
      "detail": { "used": 42, "limit": 100, "resetTime": "2026-06-13T15:00:00Z" }
    }
  ],
  "usage": { "used": 120, "limit": 500, "resetTime": "2026-06-17T00:00:00Z" },
  "totalQuota": { "limit": 1000, "remaining": 830 },
  "user": { "region": "us", "membership": { "level": "pro" } }
}
```

The Kimi example maps the 300-minute `limits[].detail` entry to a `5h` window, maps `usage` to a weekly window, and derives total quota usage from `totalQuota.limit - totalQuota.remaining`.

Example raw Codex usage response:

```json
{
  "plan_type": "plus",
  "credits": { "granted": 100, "used": 12 },
  "rate_limit": {
    "primary_window": { "used_percent": 32, "reset_after_seconds": 7200 },
    "secondary_window": { "used_percent": 18, "reset_at": 1781913600 }
  }
}
```

The Codex example reports percentages rather than absolute counters, so `used` and `limit` stay `null`, `used_percent` becomes `usedPercent`, and reset values are converted from seconds or relative seconds into ISO timestamps.

Example raw DeepSeek balance response:

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

DeepSeek is pay-as-you-go, so the API reports the current balance instead of a quota window. The example maps each `balance_infos[]` entry to a window with `remaining` set to `total_balance` and `unit` set to the currency. Optional local env vars can provide user-specific display context:

```text
DEEPSEEK_BALANCE_REFERENCE_TOTAL_CNY=200
DEEPSEEK_BALANCE_WARNING_CNY=20
DEEPSEEK_BALANCE_CURRENCY=CNY
```

The reference total lets QuotaBarWin render a percentage progress bar. The warning amount triggers an absolute low-balance warning when the current balance is at or below that value. Currency-specific variables such as `_CNY` override the generic `DEEPSEEK_BALANCE_REFERENCE_TOTAL` and `DEEPSEEK_BALANCE_WARNING` values.

### Local config and secrets

Remote provider source should not contain credentials. The script still reads `process.env.NAME`, but QuotaBarWin injects configured environment variables only into the child process. For each manifest `requiredEnvVars` entry, the app first checks the installed provider config `envVars` map; if a key is absent, it resolves `${secret:NAME}`. Extra configured `envVars` are also injected, which is useful for optional provider settings such as reference totals, currency filters, or warning thresholds.

`${secret:NAME}` reads `<config-dir>/secrets/NAME.txt` first and falls back to environment variable `NAME`. Existing `${file:C:\path\secret.txt}` and `${env:NAME}` placeholders are still supported.

Example installed provider config:

```json
{
  "kind": "remote",
  "id": "kimi-coding",
  "envVars": {
    "KIMI_API_KEY": "${secret:KIMI_API_KEY}"
  }
}
```

Treat the remote script as shared code and keep user-specific tokens in local config, local secret files, or environment variables on the local machine.

## Security checklist

- Only install remote providers from sources you trust.
- Review each provider's `sourceUrl`, `runtime`, and `requiredEnvVars` before installing a registry.
- Prefer registries that include `providers[].checksum` so QuotaBarWin can verify the manifest before installing.
- Prefer manifests that include `checksums.source`; without it QuotaBarWin cannot auto-update safely.
- The cached source file lives in the app data directory under `providers/remote/<id>/`.

## Examples

See [`examples/remote-providers/`](../examples/remote-providers) for complete sample providers:

- `kimi-coding` — Kimi coding quota via `KIMI_API_KEY`.
- `bigmodel-coding-plan` — Zhipu/BigModel quota via `BIGMODEL_API_KEY`.
- `codex-usage` — ChatGPT/Codex 5h and weekly usage via `~/.codex/auth.json` by default, with optional `CODEX_ACCESS_TOKEN`, `CODEX_ACCOUNT_ID`, or `CODEX_AUTH_FILE` env var overrides. Supports runtime proxy injection via `QBWIN_PROXY_URL`.
- `deepseek-balance` — DeepSeek pay-as-you-go balance via `${secret:DEEPSEEK_API_KEY}` plus optional local balance display settings.

To host your own, upload a directory containing `provider.json` + the source file and paste the raw `provider.json` URL into QuotaBarWin.

## Updating a remote provider

If the manifest contains `checksums.source`, QuotaBarWin can detect when the source file changes:

- **Auto-update**: enabled per provider during install; updates are applied silently when the checksum differs.
- **Manual update**: use the "Check Updates" / "Apply Update" buttons in Settings.

If `checksums.source` is missing, updates must be applied by removing and re-adding the provider.
