# Remote Provider Examples

These are complete, hostable remote provider examples for QuotaBarWin.

## Structure

Each subdirectory contains:

- `provider.json` — the remote provider manifest.
- `provider.cjs` — the source script executed by QuotaBarWin.

`manifest.example.json` is a standalone, minimal manifest template you can copy when creating your own provider.

`registry.json` is a provider registry that lists all example providers. Paste its URL/path into **Settings → Providers → Remote Sources → Registry URL** and click **Install Registry** to install them all at once.

## Providers

| Provider | Required env var | Description |
|----------|------------------|-------------|
| `kimi-coding` | `KIMI_API_KEY` | Kimi coding quota usage. |
| `bigmodel-coding-plan` | `BIGMODEL_API_KEY` | Zhipu/BigModel coding plan quota. |
| `codex-usage` | none by default | ChatGPT/Codex 5h and weekly usage. Reads `~/.codex/auth.json` by default. Optional `CODEX_ACCESS_TOKEN`, `CODEX_ACCOUNT_ID`, or `CODEX_AUTH_FILE` env vars can override the local Codex auth file. Supports runtime proxy injection via `QBWIN_PROXY_URL`. |
| `deepseek-balance` | `DEEPSEEK_API_KEY` | DeepSeek pay-as-you-go balance. Optional `DEEPSEEK_BALANCE_REFERENCE_TOTAL`, `DEEPSEEK_BALANCE_WARNING`, and `DEEPSEEK_BALANCE_CURRENCY`; append `_CNY` or another currency code for per-currency overrides. |

## Usage

1. Upload the whole `remote-providers/` directory to a static host (e.g., GitHub Raw).
2. In QuotaBarWin, go to **Settings → Providers → Remote Sources**.
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

## Stable window IDs

Provider window IDs are user-facing configuration keys. QuotaBarWin supports `visibleWindowIds` to choose which windows are shown and to display them in the configured order, and `windowLabelOverrides` to rename windows. Label overrides match `window.id` first, so keep IDs stable across provider releases.

Use `label` for friendly UI text only. It can be clearer, localized, or renamed later; it should not be the only stable identity. Prefer IDs derived from provider API semantics, such as `5h`, `weekly`, `300-minute`, `tokens-limit-6-1`, or `total-quota`, and avoid deriving IDs from translated labels or marketing copy.

Examples:

- BigModel maps `data.limits[]` to `windows[]`, with stable IDs like `tokens-limit-6-1`, plus `currentValue -> used`, `usage -> limit`, `percentage -> usedPercent`, and `nextResetTime -> resetAt`.
- Kimi maps the 300-minute `limits[].detail` entry to id `300-minute` with label `5h`, maps `usage` to id `usage` with label `Weekly limit`, and derives total quota usage from `totalQuota.limit - totalQuota.remaining`.
- Codex maps `rate_limit.primary_window` to id/label `5h` and `rate_limit.secondary_window` to id `weekly` with label `Weekly limit`; because the API reports percentages, `used` and `limit` remain `null`.
- DeepSeek maps each `balance_infos[]` currency to a stable id like `balance-cny`. Because the API reports current balance rather than a quota limit, set optional local env vars such as `DEEPSEEK_BALANCE_REFERENCE_TOTAL_CNY=200` and `DEEPSEEK_BALANCE_WARNING_CNY=20` when you want a progress bar and absolute low-balance warning.

## Updating checksums

If you edit a source script, recompute the SHA-256 checksum and update `provider.json`:

```bash
sha256sum provider.cjs
```

Then set `checksums.source` to `sha256:<hex>`.

If you edit `provider.json` itself, including display metadata such as `version`,
also recompute that manifest checksum and update `registry.json`.
