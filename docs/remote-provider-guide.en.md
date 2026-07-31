# QuotaBarWin Remote Provider Guide

Default documentation is Simplified Chinese:
[`remote-provider-guide.md`](remote-provider-guide.md).

Remote providers let you install quota providers from a hosted manifest + script, without bundling them into the app. This is useful for third-party providers, rapid iteration, or sharing provider configurations across machines.

## How it works

1. You provide a registry URL or local file path (`registry.json`) that lists one or more providers. Both `https://` and `file://` URLs, as well as plain local paths like `C:\Providers\registry.json`, are supported.
2. QuotaBarWin reads the registry, fetches each referenced provider manifest, verifies optional checksums, and downloads the source scripts.
3. Each script is cached locally and executed with its declared runtime. `builtin-js`
   uses the app's embedded QuickJS and needs no user-installed Node.js; `node`,
   `python`, `pwsh`, `bash`, and absolute paths still start an external process.
4. On every refresh QuotaBarWin runs the cached scripts and normalizes their results into quota windows.

The same manifest can be installed more than once to query multiple accounts
for the same provider. QuotaBarWin generates a stable local provider id for
each account instance, such as `kimi-coding` and `kimi-coding-2`, and each
instance can have its own name, environment variables, timeout, and window
display preferences. Configure runtime proxy through Provider environment
variables; the project-wide proxy is only a fallback when that value is absent.

## Manifest format (`provider.json`)

```json
{
  "schemaVersion": 2,
  "id": "kimi-coding",
  "displayName": "Kimi Coding Usage",
  "version": "1.2.0",
  "minAppVersion": "1.1.0",
  "description": "Kimi coding quota usage via remote provider script",
  "runtime": "builtin-js",
  "entry": "provider.js",
  "requiredEnvVars": ["KIMI_API_KEY"],
  "output": "provider-snapshot-v1",
  "permissions": ["env:KIMI_API_KEY", "net:https://api.kimi.com"],
  "defaultConfig": {
    "name": "Kimi",
    "visibleWindowIds": ["300-minute", "usage"],
    "windowLabelOverrides": {
      "300-minute": "5h",
      "usage": "Weekly"
    },
    "envVars": {
      "KIMI_API_KEY": "${secret:KIMI_API_KEY}"
    }
  },
  "parameters": [
    {
      "name": "KIMI_API_KEY",
      "label": "Kimi API Key",
      "kind": "secret",
      "required": true,
      "defaultValue": "${secret:KIMI_API_KEY}",
      "description": "Kimi coding quota API token.",
      "helpUrl": "https://provider.example.com/api-keys"
    }
  ],
  "checksums": {
    "source": "sha256:<hex>"
  }
}
```

Field descriptions:

| Field | Required | Description |
|-------|----------|-------------|
| `schemaVersion` | yes | `1` is the legacy format. The current app still loads an already cached schema 1 `builtin-js` script for upgrade continuity, but a remotely installed or updated `builtin-js` Provider must use `2` so older apps safely reject it before downloading its script. |
| `id` | yes | Stable manifest id. When the same manifest is installed more than once, QuotaBarWin generates a non-conflicting local provider id. |
| `displayName` | yes | Human-readable name shown in the UI. |
| `version` | no | Human-readable provider version shown in Settings. SemVer is recommended. If omitted, the UI falls back to a short checksum. |
| `description` | no | Short description. |
| `minAppVersion` | yes for schema 2 | Minimum QuotaBarWin SemVer version, without a `v` prefix, that can run this Provider; for example `1.1.0`. A lower host rejects the update without changing its cache. |
| `runtime` | yes | Runtime used to execute `entry`. `builtin-js` uses embedded QuickJS; `node`, `python`, `pwsh`, `bash`, and absolute executable paths remain supported. |
| `entry` | yes | Source file name. Can be a relative path (resolved against the manifest URL/directory), an absolute HTTPS URL, a `file://` URL, or a local file path. |
| `requiredEnvVars` | no | Environment variables that the script needs. On refresh, QuotaBarWin resolves each name from provider `envVars`, then `${secret:NAME}`. |
| `output` | yes | Output contract. Only `provider-snapshot-v1` is supported for remote providers at the moment. |
| `permissions` | no | Informational for external runtimes; an enforced capability boundary for `builtin-js`. See the next section for its syntax. |
| `defaultConfig` | no | Default local provider config written during first install, such as `name`, `timeoutSeconds`, `visibleWindowIds`, `windowLabelOverrides`, and `envVars`. Provider updates do not overwrite user edits. |
| `parameters` | no | Structured parameters shown in Settings. Each item may include `name`, `label`, `kind`, `required`, `defaultValue`, `placeholder`, `description`, `options`, `helpUrl`, and `advanced`. Do not include real credentials. |
| `checksums.source` | no | SHA-256 checksum of the source file. Required if you want `autoUpdate` to work. Format: `sha256:<hex>`. `version` is display metadata and does not replace checksum verification. |

### Guided setup fields

The current app generates the post-install setup form from `parameters`: `secret` uses a password input, `string` a text input, `number` a numeric input, and `select` a select control. Required fields are validated first, and saved secret values are never returned to or shown by the frontend.

`helpUrl` links to API-key or parameter help. Set `"advanced": true` to keep optional, proxy, or diagnostic fields under **Advanced settings** by default; do not mark a normal first-time requirement as advanced.

New instances install as disabled and “Pending setup”. When the user chooses **Save and test**, the host writes secret input to an instance-isolated local file and stores `${secret:providers/<provider-instance-id>/<parameter-name>}` in main config. Resolution still uses the existing `${secret:...}` resolver and the formal Provider runner. Provider authors must not require users to create files or type placeholders manually.

Third-party manifests without `parameters` still install and use the retained raw `envVars` editor, preserving old-manifest compatibility.

## Embedded JavaScript runtime (`builtin-js`)

`builtin-js` is for Providers that should not require end users to install
Node.js. It runs inside the app's embedded QuickJS sandbox. A newly published or
remotely updated manifest must use `schemaVersion: 2` and declare `minAppVersion`;
the entry must be a
`.js` file, `output` must be `provider-snapshot-v1`, and the script defines a
synchronous global `main(qb)` function. That function returns the snapshot
object directly; it does **not** use `console.log`, stdout, or `process.exit`.
All project-maintained Providers use this runtime.

```js
function main(qb) {
  const token = qb.env.get("EXAMPLE_API_TOKEN");
  const response = qb.http.request("https://api.example.com/usage", {
    headers: { Authorization: `Bearer ${token}` }
  });
  if (!response.ok) throw new Error(`Usage API returned ${response.status}`);
  const raw = JSON.parse(response.body);
  qb.log({ level: "info", stage: "snapshot.ready", message: "Usage loaded" });
  return {
    status: "ok",
    updatedAt: qb.now(),
    windows: [{ id: "monthly", label: "Monthly", remainingPercent: raw.remaining, confidence: "exact" }]
  };
}
```

The manifest must explicitly grant every host capability the script uses:

```json
{
  "schemaVersion": 2,
  "minAppVersion": "1.1.0",
  "runtime": "builtin-js",
  "entry": "provider.js",
  "output": "provider-snapshot-v1",
  "requiredEnvVars": ["EXAMPLE_API_TOKEN"],
  "permissions": ["env:EXAMPLE_API_TOKEN", "net:https://api.example.com"]
}
```

### `qb` API and permissions

| API | Required permission | Behavior and limit |
|---|---|---|
| `qb.env.get(name)` | `env:NAME` or `env-prefix:PREFIX_` | Reads a configured value and throws when it is absent. Every `requiredEnvVars` item needs a matching env permission. |
| `qb.env.getOptional(name)` | same | Returns `null` when the value is absent, but still throws for an undeclared name. Use for optional filters or thresholds. |
| `qb.fs.readText(path)` | `fs:C:\exact\path`, `fs:~/.codex/auth.json`, or `fs:env:NAME` | Reads UTF-8 text only. The host canonicalizes the real path and rejects ungranted paths. `fs:env:NAME` permits the one file named by that environment value. |
| `qb.http.request(url, options)` | `net:http`, `net:https`, or an exact origin such as `net:https://api.example.com` | Synchronous HTTP request returning `{ status, ok, body }`. The host uses the Provider or project proxy; the script never receives proxy credentials. Request bodies are limited to 1 MiB and response text to 2 MiB. |
| `qb.now()` / `qb.timezone()` | none | Current UTC ISO timestamp and the machine UTC offset, such as `UTC+08:00`. |
| `qb.log(entry)` | none | Writes a structured local app log. Prefer `{ level, stage, message }`; never pass secrets, tokens, or full responses. |
| `qb.meta` | none | Read-only generic metadata: `providerId`, `manifestId`, `name`, `version`, `sourceChecksum`, and `timeoutSeconds`. |

`net:https://api.example.com` permits only that scheme, host, and port; the
script chooses the path. `net:https` permits any HTTPS origin and should be
used only when necessary. `env-prefix:` supports optional dynamically named
settings such as `DEEPSEEK_BALANCE_WARNING_` per currency. `QBWIN_` is a
reserved host prefix and cannot be granted through `env:` or `fs:env:`; use
`qb.meta` for generic metadata and let `qb.http` use the proxy automatically.

### Supported scope and intentional exclusions

Use synchronous standard JavaScript plus `Date`, `JSON`, `RegExp`, `Map`, and
`Set`. `main(qb)` must synchronously return a JSON-serializable object.
Promises, top-level await, ES modules/import, `require`, Node/Bun/Deno APIs,
`process`, `console`, subprocesses, arbitrary sockets, arbitrary file access,
and dynamic loading are unavailable. `eval` is not a supported capability.
The runtime is limited to 16 MiB memory, a 512 KiB JS stack, and the instance's
overall `timeoutSeconds`; each HTTP request is also limited by remaining total
time.

These are platform capabilities only, not provider-specific helpers. URLs,
headers, local auth-file formats, and conversion to `windows[]` remain in the
Provider script. External runtimes remain available but do not receive this
enforced sandbox.

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

## Validate a Provider with the CLI

After writing or updating a Provider, validate its manifest, source, checksum,
and runtime with `QuotaBarWin.Cli.exe validate` before installing or publishing
it. The command always writes JSON to stdout, so it also works well in CI and
scripts that branch on its exit code.

```powershell
# Validate a local manifest. A relative local entry automatically locates its source.
.\QuotaBarWin.Cli.exe validate --manifest .\provider.json

# If entry is an HTTPS or file URL, specify the local source to validate.
.\QuotaBarWin.Cli.exe validate --manifest .\provider.json --source .\provider.js

# After installation, validate the effective config, cached manifest, source checksum, and runtime.
.\QuotaBarWin.Cli.exe validate --provider my-provider

# Also run the installed script and validate its provider-snapshot-v1 output; this can call account APIs.
.\QuotaBarWin.Cli.exe validate --provider my-provider --run
```

Validation does not run a script or make Provider API requests unless `--run` is
specified. `--run` is available only for an installed Provider and uses that
instance's configured runtime, secret placeholders, and proxy. Do not put
tokens, cookies, API keys, or proxy credentials on the command line.

Validation checks `schemaVersion`, a non-empty `displayName`, the `output`
protocol, runtime availability, source presence, `checksums.source`, and the
configured required environment-variable names for an installed Provider.
`checksums.source` remains optional in the current public contract: a missing
value is a warning, not a validation failure by itself. With `--run`, the CLI
also confirms that stdout is accepted by the current `provider-snapshot-v1`
parser; Provider stderr, secrets, and environment-variable values are not
included in the report.

A validation failure exits with code `30`; command-level failures such as an
unreadable manifest or config exit with code `20`. See [the CLI guide](cli.en.md)
for the complete protocol.

## Source script output contract

When `output` is `provider-snapshot-v1`, an external-runtime script must print
one JSON object to stdout, and stdout must contain only that final object;
write progress, debug, and error logs to stderr. `builtin-js` does not use
stdout/stderr: its `main(qb)` returns the same-shaped object and calls `qb.log()`
for logs. `id`, `name`, and `source` are optional; even when provided,
QuotaBarWin prefers the local installed provider id and name so one manifest can
back multiple account instances.

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

### Flow logs and host metadata

QuotaBarWin reads script stderr line by line and forwards it to the local app
log, `quotabarwin.log`. If the script fails, exits non-zero, or is killed by
the host timeout, the most recent roughly 16 KiB of stderr summary is also
stored in the provider diagnostics `stderr` field. This makes it easier to tell
whether the failure came from provider code or from the host timeout. Plain text
is forwarded too, but structured JSON lines are recommended.

Recommended fields:

| Field | Description |
|-------|-------------|
| `level` | `debug`, `info`, `warn`, or `error`; app log writes still respect the configured app `logLevel`. |
| `stage` | Current stage, for example `auth.loaded`, `usage.request.start`, `usage.response`, or `snapshot.ready`. |
| `message` | Short human-readable message. |
| `providerId` | Optional; host log entries already include the provider id. |
| `version` | Optional; read `QBWIN_PROVIDER_VERSION` when available. |
| `sourceChecksum` | Optional; read `QBWIN_PROVIDER_SOURCE_CHECKSUM` when available. Host logs show a short checksum. |
| Extra fields | Prefer booleans, numbers, and short strings. Nested objects are omitted from the log summary. |

The host injects these reserved environment variables before running the script:

| Environment variable | Description |
|----------------------|-------------|
| `QBWIN_PROVIDER_ID` | Provider id from the installed config. |
| `QBWIN_PROVIDER_MANIFEST_ID` | Provider id declared by the manifest. |
| `QBWIN_PROVIDER_NAME` | UI display name. |
| `QBWIN_PROVIDER_VERSION` | Manifest `version`; omitted when not set. |
| `QBWIN_PROVIDER_SOURCE_CHECKSUM` | Manifest `checksums.source`; omitted when not set. |
| `QBWIN_PROVIDER_TIMEOUT_SECONDS` | Current host timeout in seconds. |
| `QBWIN_PROXY_URL` | Optional; the Provider environment value takes priority. When absent, the host injects the project-wide proxy. An installation-source proxy is only used to download registries, manifests, and scripts. Log only whether it is enabled or its protocol, never the full value. |

Node.js example:

```js
function logStep(level, stage, message, fields = {}) {
  process.stderr.write(`${JSON.stringify({
    level,
    providerId: process.env.QBWIN_PROVIDER_ID || "my-provider",
    version: process.env.QBWIN_PROVIDER_VERSION || null,
    sourceChecksum: process.env.QBWIN_PROVIDER_SOURCE_CHECKSUM || null,
    stage,
    message,
    ...fields,
  })}\n`);
}

logStep("info", "usage.request.start", "Fetching usage", {
  timeoutSeconds: process.env.QBWIN_PROVIDER_TIMEOUT_SECONDS || null,
});

console.log(JSON.stringify({
  status: "ok",
  updatedAt: new Date().toISOString(),
  windows: [],
  metadata: {},
}));
```

If a provider sets its own HTTP or CLI timeout, prefer a value lower than
`QBWIN_PROVIDER_TIMEOUT_SECONDS`. That gives the script time to emit a
`level=error` failure log and exit normally; otherwise the host can only record
`timeoutOrigin=host`. Never log tokens, API keys, cookies, authorization
headers, account IDs, proxy credentials, or full proxy URLs. The host applies
basic redaction, but providers should avoid emitting sensitive values in the
first place.

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
  "id": "kimi-coding-2",
  "name": "Kimi Coding - Work",
  "timeoutSeconds": 30,
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
- `timeoutSeconds` controls how long the QuotaBarWin host process waits for the script to exit. The default is `30` seconds.
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

The Codex example identifies its 5h and weekly windows from `limit_window_seconds`, avoiding an API swap of `primary_window` / `secondary_window`; when that field is absent, it retains the legacy positional mapping. It reports percentages rather than absolute counters, so `used` and `limit` stay `null`, `used_percent` becomes `usedPercent`, and reset values are converted from seconds or relative seconds into ISO timestamps.

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

Remote provider source should not contain credentials. External-runtime scripts
read `process.env.NAME`; `builtin-js` scripts use permission-gated
`qb.env.get("NAME")` or `qb.env.getOptional("NAME")`. For each manifest
`requiredEnvVars` entry, the app first checks the installed provider config
`envVars` map; if a key is absent, it resolves `${secret:NAME}`. Extra configured
`envVars` are also available to the Provider, which is useful for optional
reference totals, currency filters, or warning thresholds.

For an installation form backed by `parameters`, a secret entered by a normal user is automatically written to:

```text
<config-dir>/secrets/providers/<provider-instance-id>/<parameter-name>.txt
```

The matching `envVars` entry stores only `${secret:providers/<provider-instance-id>/<parameter-name>}`. This is a local plaintext file, not system credential storage; it moves with a portable directory, and the app does not place its contents in config, logs, diagnostics, or form read-back. This application-managed form is read through the existing `${secret:...}` resolver described below.

`${secret:NAME}` reads `<config-dir>/secrets/NAME.txt` first and falls back to environment variable `NAME`. Existing `${file:C:\path\secret.txt}` and `${env:NAME}` placeholders are still supported.

For a single account, if the manifest declares `requiredEnvVars:
["KIMI_API_KEY"]` and you leave provider `envVars` empty, QuotaBarWin tries this
default file:

```text
<config-dir>/secrets/KIMI_API_KEY.txt
```

For multiple accounts, explicitly set the mapping in each local provider
instance under **Settings → Providers → Edit → Environment variables**. The left
side of `=` stays the environment variable name that the script expects; the
right side is the local secret file name for that account.

For example, two accounts using the same Kimi provider:

```json
{
  "kind": "remote",
  "id": "kimi-coding",
  "name": "Kimi Personal",
  "envVars": {
    "KIMI_API_KEY": "${secret:KIMI_API_KEY_PERSONAL}"
  }
}
```

```json
{
  "kind": "remote",
  "id": "kimi-coding-2",
  "name": "Kimi Work",
  "envVars": {
    "KIMI_API_KEY": "${secret:KIMI_API_KEY_WORK}"
  }
}
```

The corresponding local files are:

```text
<config-dir>/secrets/KIMI_API_KEY_PERSONAL.txt
<config-dir>/secrets/KIMI_API_KEY_WORK.txt
```

Both instances inject `process.env.KIMI_API_KEY` into the script, but the values
come from different secret files. The config stores only placeholders, not plain
tokens.

Treat the remote script as shared code and keep user-specific tokens in local config, local secret files, or environment variables on the local machine.

## Implementation examples (Zhipu/BigModel)

These examples call the Zhipu/BigModel quota endpoint and print a
`provider-snapshot-v1` response. They all read the API key from
`BIGMODEL_API_KEY`.

### Node.js

```js
const token = process.env.BIGMODEL_API_KEY;
if (!token) {
  console.error("BIGMODEL_API_KEY is required");
  process.exit(1);
}

fetch("https://open.bigmodel.cn/api/monitor/usage/quota/limit", {
  headers: { Authorization: `Bearer ${token}` },
})
  .then((res) => {
    if (!res.ok) throw new Error(`HTTP ${res.status}`);
    return res.json();
  })
  .then((body) => {
    const limits = body.data?.limits ?? [];
    const windows = limits.map((l) => ({
      id: `${l.type}-${l.unit}-${l.number}`.toLowerCase(),
      label: `${l.type} / ${l.unit}`,
      used: l.currentValue ?? null,
      limit: l.usage ?? null,
      unit: l.type === "TOKENS_LIMIT" ? "tokens" : null,
      usedPercent: l.percentage ?? null,
      remainingPercent:
        l.percentage == null ? null : Math.max(0, 100 - l.percentage),
      resetAt: l.nextResetTime
        ? new Date(l.nextResetTime).toISOString()
        : null,
      resetText: null,
      confidence: "exact",
    }));
    console.log(JSON.stringify({
      status: "ok",
      updatedAt: new Date().toISOString(),
      windows,
      metadata: {},
    }));
  })
  .catch((err) => {
    console.error(err.message);
    process.exit(1);
  });
```

### Python

```python
import json, os, sys, urllib.request, datetime

token = os.environ.get("BIGMODEL_API_KEY")
if not token:
    sys.exit("BIGMODEL_API_KEY is required")

req = urllib.request.Request(
    "https://open.bigmodel.cn/api/monitor/usage/quota/limit",
    headers={"Authorization": f"Bearer {token}"}
)

with urllib.request.urlopen(req) as res:
    body = json.load(res)

limits = body.get("data", {}).get("limits", [])
windows = []
for l in limits:
    pct = l.get("percentage")
    windows.append({
        "id": f"{l['type']}-{l['unit']}-{l['number']}".lower(),
        "label": f"{l['type']} / {l['unit']}",
        "used": l.get("currentValue"),
        "limit": l.get("usage"),
        "unit": "tokens" if l.get("type") == "TOKENS_LIMIT" else None,
        "usedPercent": pct,
        "remainingPercent": None if pct is None else max(0, 100 - pct),
        "resetAt": datetime.datetime.fromtimestamp(
            l["nextResetTime"] / 1000, tz=datetime.timezone.utc
        ).isoformat() if l.get("nextResetTime") else None,
        "resetText": None,
        "confidence": "exact"
    })

print(json.dumps({
    "status": "ok",
    "updatedAt": datetime.datetime.now(datetime.timezone.utc).isoformat(),
    "windows": windows,
    "metadata": {}
}, ensure_ascii=False))
```

### PowerShell

```powershell
$token = $env:BIGMODEL_API_KEY
if (-not $token) { throw "BIGMODEL_API_KEY is required" }

$res = Invoke-RestMethod -Uri "https://open.bigmodel.cn/api/monitor/usage/quota/limit" -Headers @{
  Authorization = "Bearer $token"
}

$windows = @()
foreach ($l in $res.data.limits) {
  $pct = $l.percentage
  $windows += @{
    id = "$($l.type)-$($l.unit)-$($l.number)".ToLower()
    label = "$($l.type) / $($l.unit)"
    used = $l.currentValue
    limit = $l.usage
    unit = if ($l.type -eq "TOKENS_LIMIT") { "tokens" } else { $null }
    usedPercent = $pct
    remainingPercent = if ($null -eq $pct) { $null } else { [math]::Max(0, 100 - $pct) }
    resetAt = if ($l.nextResetTime) {
      ([DateTimeOffset]::FromUnixTimeMilliseconds($l.nextResetTime).UtcDateTime).ToString("o")
    } else { $null }
    resetText = $null
    confidence = "exact"
  }
}

@{
  status = "ok"
  updatedAt = (Get-Date).ToString("o")
  windows = $windows
  metadata = @{}
} | ConvertTo-Json -Depth 10
```

### Bash (Git Bash)

```bash
#!/usr/bin/env bash
set -euo pipefail

TOKEN="${BIGMODEL_API_KEY:-}"
[ -z "$TOKEN" ] && { echo "BIGMODEL_API_KEY is required" >&2; exit 1; }

RESPONSE=$(curl -fsS -H "Authorization: Bearer $TOKEN" \
  "https://open.bigmodel.cn/api/monitor/usage/quota/limit")

UPDATED_AT=$(date -u +"%Y-%m-%dT%H:%M:%SZ")

WINDOWS=$(echo "$RESPONSE" | jq '[.data.limits[] | {
  id: "\(.type)-\(.unit)-\(.number)" | ascii_downcase,
  label: "\(.type) / \(.unit)",
  used: .currentValue,
  limit: .usage,
  unit: (if .type == "TOKENS_LIMIT" then "tokens" else null end),
  usedPercent: .percentage,
  remainingPercent: (if .percentage == null then null else [0, 100 - .percentage] | max end),
  resetAt: (if .nextResetTime == null then null else (.nextResetTime / 1000 | strflocaltime("%Y-%m-%dT%H:%M:%SZ")) end),
  resetText: null,
  confidence: "exact"
}]')

jq -n --arg updatedAt "$UPDATED_AT" --argjson windows "$WINDOWS" \
  '{status: "ok", updatedAt: $updatedAt, windows: $windows, metadata: {}}'
```

For Bash, `jq` must be available. On Windows, Git Bash usually ships with it.

## Security checklist

- **Only install and use remote providers you trust. Provider scripts can directly
  read the AI credentials configured for them and make network requests.**
- Review each provider's `sourceUrl`, `runtime`, and `requiredEnvVars` before installing a registry.
- Prefer registries that include `providers[].checksum` so QuotaBarWin can verify the manifest before installing.
- Prefer manifests that include `checksums.source`; without it QuotaBarWin cannot auto-update safely.
- The cached source file lives in the app data directory under `providers/remote/<id>/`, where `<id>` is the local installed instance id. Multiple accounts from the same manifest use different directories.

## Examples

See [`examples/remote-providers/`](../examples/remote-providers) for complete sample providers:

- `kimi-coding` — Kimi coding quota via `KIMI_API_KEY`.
- `bigmodel-coding-plan` — Zhipu/BigModel quota via `BIGMODEL_API_KEY`.
- `codex-usage` — ChatGPT/Codex 5h and weekly usage via `~/.codex/auth.json` by default, with optional `CODEX_ACCESS_TOKEN`, `CODEX_ACCOUNT_ID`, or `CODEX_AUTH_FILE` env var overrides. Supports runtime proxy injection via `QBWIN_PROXY_URL`.
- `deepseek-balance` — DeepSeek pay-as-you-go balance via `${secret:DEEPSEEK_API_KEY}` plus optional local balance display settings.

To host your own, upload a directory containing `provider.json` + the source file and paste the raw `provider.json` URL into QuotaBarWin.

## Updating a remote provider

If the manifest contains `checksums.source`, QuotaBarWin can detect when the source file changes. Settings shows the installed version, install time, update time, and last check time:

- **Auto-update**: enabled per provider during install; updates are applied silently when the checksum differs.
- **Manual update**: use the "Check Updates" / "Apply Update" buttons in Settings.

If `checksums.source` is missing, updates must be applied by removing and re-adding the provider.

### App-version compatibility

Publish or remotely update a `builtin-js` Provider with manifest schema 2 and the
actual minimum `minAppVersion`. Before an update, the host validates both fields. If the app is
too old, it keeps the current cache, config, and source untouched and tells the
user to update QuotaBarWin first. Older apps understand only schema 1, so they
also reject a schema 2 manifest **before** downloading the new source. This
prevents automatic updates from replacing a working Node Provider with a
`builtin-js` script that an older host cannot run. For a smooth migration, the
current app still runs an already cached schema 1 `builtin-js` Provider, but
will not install or update one from a remote manifest.
