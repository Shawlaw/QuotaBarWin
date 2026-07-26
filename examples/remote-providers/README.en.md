# Remote Provider Examples

Default documentation is Simplified Chinese: [`README.md`](README.md).

These are complete, hostable remote provider examples for QuotaBarWin.

## Structure

Each subdirectory contains:

- `provider.json` — the remote provider manifest.
- `provider.js` — the `builtin-js` source script used by project-maintained Providers.
- `api.md` — request, response, field mapping, and fixture notes for the provider.
- `api.en.md` — English version of `api.md`.

`manifest.example.json` is a standalone, minimal manifest template you can copy when creating your own provider.

`registry.json` is a provider registry that lists all example providers. Paste its URL/path into **Settings → Providers → Remote Sources → Registry URL** and click **Install Registry** to install them all at once.

## Providers

The top-level README is only an index. Each provider directory's `api.md` records the request, response, and field mapping used by the current example script. Update that API note whenever `provider.js` changes.

| Provider | Data source | Required env var | API notes | Description |
|----------|-------------|------------------|-----------|-------------|
| `kimi-coding` | `GET https://api.kimi.com/coding/v1/usages` | `KIMI_API_KEY` | [`api.md`](kimi-coding/api.md) / [`EN`](kimi-coding/api.en.md) | Kimi coding quota usage. |
| `bigmodel-coding-plan` | `GET https://open.bigmodel.cn/api/monitor/usage/quota/limit` | `BIGMODEL_API_KEY` | [`api.md`](bigmodel-coding-plan/api.md) / [`EN`](bigmodel-coding-plan/api.en.md) | Zhipu/BigModel coding plan quota. |
| `codex-usage` | `GET https://chatgpt.com/backend-api/wham/usage` | none by default | [`api.md`](codex-usage/api.md) / [`EN`](codex-usage/api.en.md) | ChatGPT/Codex 5h and weekly usage. Reads `~/.codex/auth.json` by default. Optional `CODEX_ACCESS_TOKEN`, `CODEX_ACCOUNT_ID`, or `CODEX_AUTH_FILE` env vars can override the local Codex auth file. Supports runtime proxy injection via `QBWIN_PROXY_URL`. |
| `deepseek-balance` | `GET https://api.deepseek.com/user/balance` | `DEEPSEEK_API_KEY` | [`api.md`](deepseek-balance/api.md) / [`EN`](deepseek-balance/api.en.md) | DeepSeek pay-as-you-go balance. Optional `DEEPSEEK_BALANCE_REFERENCE_TOTAL`, `DEEPSEEK_BALANCE_WARNING`, and `DEEPSEEK_BALANCE_CURRENCY`; append `_CNY` or another currency code for per-currency overrides. |

## Usage

1. Upload the whole `remote-providers/` directory to a static host (e.g., GitHub Raw).
2. In QuotaBarWin, go to **Settings → Providers → Remote Sources**.
3. Paste the raw URL of `registry.json` into **Registry URL** and click **Install Registry**.
4. Review the install summary.

For this repository, the hosted example registry URL is:

```text
https://raw.githubusercontent.com/Shawlaw/QuotaBarWin/main/examples/remote-providers/registry.json
```

For the manifest format and output contract, see [`docs/remote-provider-guide.md`](../../docs/remote-provider-guide.md).

## Parsing pattern

Each `provider.js` keeps provider-specific API parsing local to the script and returns the normalized `provider-snapshot-v1` object from `main(qb)`. They use manifest-permission-gated `qb.env`, `qb.fs`, and `qb.http` host capabilities and do not depend on Node.js.

Use this pattern when adapting the examples:

1. Fetch the raw API response with `qb.http.request()`; for a local file, declare and use an exact `fs:` permission.
2. Add a short comment showing the raw response shape that the parser expects.
3. Convert raw quota records into `windows[]` entries with stable `id`, readable `label`, numeric `used`/`limit` values where available, percentages, and ISO reset times.
4. Move provider-specific details such as plan level, model usage, account metadata, or raw status codes into `metadata`.
5. Keep credentials local. These examples call `qb.env.get("NAME")` for manifest-declared variables; QuotaBarWin can resolve values from installed provider `envVars`, `${secret:NAME}` files under `<config-dir>/secrets/NAME.txt`, or environment fallback instead of embedding secrets in remote source.

For multiple accounts on the same provider, each local account instance still
injects the env var name expected by the script, but can map it to a different
secret file, such as `KIMI_API_KEY=${secret:KIMI_WORK_API_KEY}`. See the full
example in [`docs/remote-provider-guide.en.md`](../../docs/remote-provider-guide.en.md#local-config-and-secrets).

## Stable window IDs

Provider window IDs are user-facing configuration keys. QuotaBarWin supports `visibleWindowIds` to choose which windows are shown and to display them in the configured order, and `windowLabelOverrides` to rename windows. Label overrides match `window.id` first, so keep IDs stable across provider releases.

Use `label` for friendly UI text only. It can be clearer, localized, or renamed later; it should not be the only stable identity. Prefer IDs derived from provider API semantics, such as `5h`, `weekly`, `300-minute`, `tokens-limit-6-1`, or `total-quota`, and avoid deriving IDs from translated labels or marketing copy.

Examples:

- BigModel maps `data.limits[]` to `windows[]`, with stable IDs like `tokens-limit-6-1`, plus `currentValue -> used`, `usage -> limit`, `percentage -> usedPercent`, and `nextResetTime -> resetAt`.
- Kimi maps the 300-minute `limits[].detail` entry to id `300-minute` with label `5h`, maps `usage` to id `usage` with label `Weekly limit`, and derives total quota usage from `totalQuota.limit - totalQuota.remaining`.
- Codex prefers `limit_window_seconds`: 18000 seconds maps to id/label `5h`, and 604800 seconds maps to id `weekly` with label `Weekly limit`, avoiding an API swap of primary / secondary positions. When the field is absent, it retains the legacy primary/secondary mapping. Because the API reports percentages, `used` and `limit` remain `null`.
- DeepSeek maps each `balance_infos[]` currency to a stable id like `balance-cny`. Because the API reports current balance rather than a quota limit, set optional local env vars such as `DEEPSEEK_BALANCE_REFERENCE_TOTAL_CNY=200` and `DEEPSEEK_BALANCE_WARNING_CNY=20` when you want a progress bar and absolute low-balance warning.

## Updating checksums

If you edit a source script, recompute the SHA-256 checksum and update `provider.json`:

```bash
sha256sum provider.js
```

Then set `checksums.source` to `sha256:<hex>`.

If you edit `provider.json` itself, including display metadata such as `version`,
also recompute that manifest checksum and update `registry.json`.
