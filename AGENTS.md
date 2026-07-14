# AGENTS.md

This file is the handoff guide for coding agents working in this repository.
Read it before changing code.

## Project Snapshot

QuotaBarWin is a Windows-only Tauri 2 desktop app for monitoring AI usage and
quota windows. The stack is:

- Frontend: React 19, TypeScript, Vite, Vitest, Testing Library.
- Backend: Rust 2021, Tauri 2, reqwest, serde, zip.
- Packaging: Windows portable zip plus a single exe. Tauri bundling is disabled;
  do not assume MSI, NSIS, updater artifacts, or signing are part of the current
  release flow.

Core invariant: **Provider is the only public data abstraction.** User-facing
providers are installed from remote registries/manifests. Remote provider
scripts can use API calls, CLI tools, JSON/text parsing, or other internals, but
the UI consumes only normalized `AppSnapshot`, `ProviderSnapshot`, and
`QuotaWindow` data.

Default project docs are Simplified Chinese. Link to English counterparts when
they exist.

## Source Of Truth

Start with these files:

- `README.md` for current user/developer-facing behavior.
- `README.en.md` only when you need the English version.
- `src/types.ts` for frontend data contracts.
- `src/lib/api.ts` for Tauri command bindings and browser-preview fallbacks.
- `src/App.tsx` for app-level refresh, cached snapshot, settings, and tray-view
  wiring.
- `src-tauri/src/lib.rs` for registered Tauri commands, startup, single
  instance behavior, hidden startup, tray setup, and background scheduler setup.
- `src-tauri/src/config.rs` for config schema, migration, portable mode, storage
  paths, startup sync, guide export, and secret placeholders.
- `src-tauri/src/quota.rs` for snapshot building, retry, stale fallback,
  on-disk snapshot cache, and provider dispatch.
- `src-tauri/src/remote_provider.rs`,
  `src-tauri/src/remote_provider_commands.rs`, and
  `src-tauri/src/remote_provider_runner.rs` for remote provider install/update,
  cache metadata, runtime resolution, execution, timeout, stderr diagnostics,
  and environment injection.
- `src-tauri/src/proxy.rs`, `src-tauri/src/logger.rs`, and
  `src-tauri/src/diagnostics.rs` for proxy priority, structured redacted logs,
  log rotation, and diagnostics export.
- `src-tauri/src/refresh_scheduler.rs` and `src-tauri/src/tray.rs` for automatic
  refresh events, tray menu, tray popup routing, position, and size persistence.
- `docs/remote-provider-guide.md`,
  `docs/remote-provider-guide.en.md`, and
  `examples/remote-providers/README.md` for the public remote provider
  contract.

Historical planning specs were removed from the open-source tree. Treat
README, current docs, and source code as the project facts.

## Current Provider Model

Current config schema version: `15`.

Supported persisted provider config kind:

- `remote`: cached external script installed from a registry or manifest.

Do not add or depend on `mock`, `codex`, `command`, or `script` provider configs
unless the task is explicitly to revive that design. Migration from schema
versions before `11` drops non-remote providers, and current config
deserialization rejects those legacy kinds. Codex usage is provided as the
`codex-usage` remote provider example.

Remote provider public contract:

- Registry schema: `schemaVersion: 1`.
- Manifest schema: `schemaVersion: 1`.
- Public output protocol: `provider-snapshot-v1`.
- Runtime can be `node`, `python`, `pwsh`, `bash`, or an absolute executable
  path. Installation resolves and validates the runtime, then stores
  `resolvedRuntime`.
- `timeoutSeconds` is part of remote provider config and defaults to `30`.
  Schema `13 -> 14` migration adds it to existing remote providers.
- `showInTray` defaults to `true`; Schema `14 -> 15` adds it to existing remote providers.
- The host injects `QBWIN_PROVIDER_ID`, `QBWIN_PROVIDER_MANIFEST_ID`,
  `QBWIN_PROVIDER_NAME`, optional version/checksum vars,
  `QBWIN_PROVIDER_TIMEOUT_SECONDS`, and optional `QBWIN_PROXY_URL`.

`remote_provider_runner.rs` has an internal parser path for `app-snapshot-v1`,
but the public docs currently only advertise `provider-snapshot-v1`. Do not
treat `app-snapshot-v1` as a supported external contract unless the task also
updates docs and tests.

## Config, Storage, And Secrets

Windows AppData config:

```text
%APPDATA%\QuotaBarWin\config.quotaBarWin.json
```

Portable config, selected when `quotabarwin.portable` exists beside the app
executable:

```text
<app-exe-dir>\config.quotaBarWin.json
```

Legacy `config.json` paths are migrated to `config.quotaBarWin.json`.

Important storage details:

- `set_portable_mode` writes or removes the `quotabarwin.portable` marker and
  copies the current config when enabling portable mode.
- `quotabarwin.log` is written beside the active config and rotates to
  `quotabarwin.log.1`; `logMaxBytes` defaults to `10 * 1024 * 1024`.
- `last_snapshot.quotaBarWin.json` is written beside the active config. Provider
  `metadata` is stripped from this disk cache.
- Installed remote provider cache paths are resolved by
  `remote_provider_commands::remote_provider_dir`; do not duplicate this path
  logic by hand. Cached provider dirs contain `provider.json`, the source file,
  `.meta.json`, and a `.bak` copy of the previous source when overwritten.

Secret placeholders:

- `${secret:NAME}` reads `<config-dir>\secrets\NAME.txt`, then env var `NAME`.
- `${env:NAME}` reads env var `NAME`.
- `${file:C:\path\secret.txt}` reads a local file and trims whitespace. Quoted
  paths with spaces are accepted.

Never hard-code real API keys, tokens, cookies, account IDs, authorization
headers, or proxy credentials in code, docs, fixtures, logs, diagnostics, or
tests. Keep diagnostics and provider stderr redacted.

## Development Commands

Install dependencies:

```powershell
npm install
```

Frontend/browser preview:

```powershell
npm run dev
```

Tauri desktop dev app:

```powershell
npm run tauri dev
```

Frontend build:

```powershell
npm run build
```

Release-style app build without bundling:

```powershell
npm run tauri -- build --no-bundle
```

Tests:

```powershell
npm run test -- --run
cargo test --manifest-path src-tauri/Cargo.toml
```

Icons:

```powershell
npm run icons:check
npm run icons:generate
```

E2E:

```powershell
npm run e2e
```

The E2E runner requires `tauri-driver`, WebDriverIO, and a release build path.
Check `e2e/tauri.e2e.mjs` before relying on it; it has historically lagged
behind provider schema changes and currently seeds legacy provider kinds.

## Testing Expectations

- Frontend changes should usually include or update Vitest tests next to the
  affected component or library.
- Rust backend changes should include focused unit tests in the touched module.
- Remote provider manifest/source changes must keep `checksums.source` in
  `provider.json` in sync. If `provider.json` changes, update the corresponding
  manifest checksum in `examples/remote-providers/registry.json`.
- `cargo test --manifest-path src-tauri/Cargo.toml` validates example provider
  manifests, source checksums, productization assumptions, release metadata, and
  many config/refresh behaviors.
- If you touch config migration, test old schema inputs and rollback behavior.
- If you touch refresh behavior, test stale fallback, disk cache behavior,
  metadata stripping, retry decisions, and single-provider refresh ordering.
- If you touch remote provider execution, test stdout parsing, stderr capture,
  timeout behavior, environment/secret injection, proxy injection, and redaction.
- If you touch tray behavior, test the tray menu labels, popup route, focus-hide
  behavior, position clamping, and size persistence.

## Coding Guidance

- Prefer existing patterns over new abstractions.
- Keep provider-specific parsing inside provider implementations or remote
  provider scripts; do not leak provider-specific fields into UI logic.
- Preserve stable `QuotaWindow.id` values. User customization depends on them.
- Keep UI rendering generic over `provider.windows`; do not hard-code `5h` and
  `weekly` as the only quota windows.
- When projecting config onto snapshots, preserve provider order, enabled state,
  `visibleWindowIds`, and `windowLabelOverrides` behavior.
- Use structured JSON parsing instead of string slicing when data is structured.
- Remote provider stdout must be the final JSON payload only. Put progress,
  debug, and failure details on stderr, preferably as one JSON object per line.
- Avoid shell command string concatenation for execution paths and arguments.
  Use structured `Command` args and env maps.
- On Windows, be careful with portable paths, AppData paths, path separators,
  hidden subprocess windows, and executable resolution.
- Keep log and diagnostics output redacted. Do not persist live provider
  `metadata` to the snapshot disk cache.
- Maintain Chinese docs first. When adding or changing public docs, update the
  English counterpart if one exists.

## Useful Local Files

- `examples/remote-providers/registry.json` installs the example remote
  providers as a registry.
- `examples/remote-providers/*/api.md` and `api.en.md` describe the API/fixture
  contract for each example provider.
- `fixtures/provider_outputs/` contains raw provider response JSON fixtures used
  by example provider docs.
- `fixtures/expected/` contains expected normalized snapshot fixtures used by
  tests.
- `fixtures/commands/` contains command fixture helpers used by older local
  command examples.
- `src-tauri/icons/source.png` is the source icon; generated icon outputs should
  be updated with `npm run icons:generate`.
- `.github/workflows/release.yml` builds the Windows portable zip
  `QuotaBarWin_<version>_windows_x64_portable_<commit>.zip` and includes the
  `quotabarwin.portable` marker.
- `src-tauri/src/productization.rs` tests the current portable-only release
  assumptions.

## Final Checks Before Handoff

Run the narrowest meaningful checks first, then broaden when risk warrants it.
For typical code changes, report whether these passed:

```powershell
npm run build
npm run test -- --run
cargo test --manifest-path src-tauri/Cargo.toml
```

If you touched release packaging, also consider:

```powershell
npm run tauri -- build --no-bundle
```

If a check cannot run locally, state why and name the residual risk.
