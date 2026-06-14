# Remote Provider Examples

These are complete, hostable remote provider examples for QuotaBarWin.

## Structure

Each subdirectory contains:

- `provider.json` — the remote provider manifest.
- `provider.cjs` — the source script executed by QuotaBarWin.

`manifest.example.json` is a standalone, minimal manifest template you can copy when creating your own provider.

`registry.json` is a provider registry that lists all three example providers. Paste its URL/path into **Settings → Remote Providers → Registry URL** and click **Install Registry** to install them all at once.

## Providers

| Provider | Required env var | Description |
|----------|------------------|-------------|
| `kimi-coding` | `KIMI_API_KEY` | Kimi coding quota usage. |
| `bigmodel-coding-plan` | `BIGMODEL_API_KEY` | Zhipu/BigModel coding plan quota. |
| `codex-usage` | `CODEX_ACCESS_TOKEN` | ChatGPT/Codex 5h and weekly usage. Prefer `${secret:CODEX_ACCESS_TOKEN}` in app config. Optional `CODEX_ACCOUNT_ID` for multi-account. |

## Usage

1. Upload the whole `remote-providers/` directory to a static host (e.g., GitHub Raw).
2. In QuotaBarWin, go to **Settings → Remote Providers**.
3. Paste the raw URL of `registry.json` into **Registry URL** and click **Install Registry**.
4. Review the install summary.

For the manifest format and output contract, see [`docs/remote-provider-guide.md`](../../docs/remote-provider-guide.md).

## Parsing pattern

Each `provider.cjs` keeps provider-specific API parsing local to the script and prints the normalized `provider-snapshot-v1` object expected by QuotaBarWin.

Use this pattern when adapting the examples:

1. Fetch the raw API response or load a fixture while testing.
2. Add a short comment showing the raw response shape that the parser expects.
3. Convert raw quota records into `windows[]` entries with stable `id`, readable `label`, numeric `used`/`limit` values where available, percentages, and ISO reset times.
4. Move provider-specific details such as plan level, model usage, account metadata, or raw status codes into `metadata`.
5. Keep credentials local. These examples read `process.env.NAME`; QuotaBarWin can inject values from installed provider `envVars`, `${secret:NAME}` files under `<config-dir>/secrets/NAME.txt`, or environment fallback instead of embedding secrets in remote source.

Examples:

- BigModel maps `data.limits[]` to `windows[]`, with `currentValue -> used`, `usage -> limit`, `percentage -> usedPercent`, and `nextResetTime -> resetAt`.
- Kimi maps the 300-minute `limits[].detail` entry to `5h`, maps `usage` to the weekly window, and derives total quota usage from `totalQuota.limit - totalQuota.remaining`.
- Codex maps `rate_limit.primary_window` to `5h` and `rate_limit.secondary_window` to `Weekly limit`; because the API reports percentages, `used` and `limit` remain `null`.

## Updating checksums

If you edit a source script, recompute the SHA-256 checksum and update `provider.json`:

```bash
sha256sum provider.cjs
```

Then set `checksums.source` to `sha256:<hex>`.
