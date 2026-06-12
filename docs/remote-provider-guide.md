# QuotaBarWin Remote Provider Guide

Remote providers let you install quota providers from a hosted manifest + script, without bundling them into the app. This is useful for third-party providers, rapid iteration, or sharing provider configurations across machines.

## How it works

1. You provide a manifest URL or local file path (a JSON file that describes the provider). Both `https://` and `file://` URLs, as well as plain local paths like `C:\Providers\provider.json`, are supported.
2. QuotaBarWin reads the manifest, computes/verifies the source checksum, and downloads the source script referenced by the manifest.
3. The script is cached locally and executed with the declared runtime (for example `node`, `python`, or an absolute path).
4. On every refresh QuotaBarWin runs the cached script and parses the output into quota windows.

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
| `requiredEnvVars` | no | Environment variables that the script needs. Shown in the install confirmation dialog. |
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
      "used": 12,
      "limit": 100,
      "unit": "requests",
      "usedPercent": 12,
      "remainingPercent": 88,
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
| `id` | yes | Stable window identifier. |
| `label` | yes | Display label. |
| `used` | no | Used amount. |
| `limit` | no | Total limit. |
| `unit` | no | Unit string, e.g. `requests`, `tokens`, `percent`. |
| `usedPercent` | no | 0-100. |
| `remainingPercent` | no | 0-100. |
| `resetAt` | no | ISO 8601 timestamp. |
| `resetText` | no | Human-readable reset text. |
| `confidence` | no | `exact`, `estimated`, or `unknown`. |

## Security checklist

- Only install remote providers from sources you trust.
- Review the manifest `sourceUrl`, `runtime`, and `requiredEnvVars` in the confirmation dialog.
- Prefer manifests that include `checksums.source`; without it QuotaBarWin cannot auto-update safely.
- The cached source file lives in the app data directory under `providers/remote/<id>/`.

## Examples

See [`examples/remote-providers/`](../examples/remote-providers) for complete sample providers:

- `kimi-coding` — Kimi coding quota via `KIMI_API_KEY`.
- `bigmodel-coding-plan` — Zhipu/BigModel quota via `BIGMODEL_API_KEY`.
- `codex-usage` — ChatGPT/Codex 5h and weekly usage via `CODEX_ACCESS_TOKEN`.

To host your own, upload a directory containing `provider.json` + the source file and paste the raw `provider.json` URL into QuotaBarWin.

## Updating a remote provider

If the manifest contains `checksums.source`, QuotaBarWin can detect when the source file changes:

- **Auto-update**: enabled per provider during install; updates are applied silently when the checksum differs.
- **Manual update**: use the "Check Updates" / "Apply Update" buttons in Settings.

If `checksums.source` is missing, updates must be applied by removing and re-adding the provider.
