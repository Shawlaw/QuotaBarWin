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
| `codex-usage` | `CODEX_ACCESS_TOKEN` | ChatGPT/Codex 5h and weekly usage. Optional `CODEX_ACCOUNT_ID` for multi-account. |

## Usage

1. Upload the whole `remote-providers/` directory to a static host (e.g., GitHub Raw).
2. In QuotaBarWin, go to **Settings → Remote Providers**.
3. Paste the raw URL of `registry.json` into **Registry URL** and click **Install Registry**.
4. Review the install summary.

For the manifest format and output contract, see [`docs/remote-provider-guide.md`](../../docs/remote-provider-guide.md).

## Updating checksums

If you edit a source script, recompute the SHA-256 checksum and update `provider.json`:

```bash
sha256sum provider.cjs
```

Then set `checksums.source` to `sha256:<hex>`.
