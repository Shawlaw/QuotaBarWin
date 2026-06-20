# AGENTS.md

This file is the handoff guide for coding agents working in this repository.
Read it before changing code.

## Project Snapshot

QuotaBarWin is a Windows-first Tauri 2 desktop app for monitoring AI usage and
quota windows. The stack is:

- Frontend: React 19, TypeScript, Vite, Vitest, Testing Library.
- Backend: Rust 2021, Tauri 2, reqwest, serde, zip.
- Packaging: Tauri MSI/NSIS plus GitHub Actions release artifacts.

Core invariant: **Provider is the only public data abstraction.** User-facing
providers are installed from remote registries/manifests. Remote provider
scripts can use API calls, CLI tools, JSON/text parsing, or other internals, but
the UI consumes only normalized `AppSnapshot`, `ProviderSnapshot`, and
`QuotaWindow` data.

## Source Of Truth

Start with these files:

- `README.md` for current user/developer-facing behavior.
- `README.en.md` only when you need the English version. Default project docs
  are Simplified Chinese and should link to English counterparts when present.
- `src/types.ts` for frontend data contracts.
- `src/lib/api.ts` for Tauri command bindings and browser-preview fallbacks.
- `src-tauri/src/lib.rs` for registered Tauri commands and app setup.
- `src-tauri/src/config.rs` for config schema, migration, storage paths, and
  secret placeholders.
- `src-tauri/src/quota.rs` for snapshot building, retry, stale fallback, and
  provider dispatch.
- `src-tauri/src/remote_provider.rs`,
  `src-tauri/src/remote_provider_commands.rs`, and
  `src-tauri/src/remote_provider_runner.rs` for remote provider install/update
  and execution.
- `docs/remote-provider-guide.md`,
  `docs/remote-provider-guide.en.md`, and
  `examples/remote-providers/README.md` for the remote provider contract.

The files under `docs/specs/` are design history and roadmap context. If a spec
conflicts with code, README, or this file, trust the code and current docs.

## Current Provider Kinds

Current config schema version: `12`.

Supported provider config kind:

- `remote`: cached script installed from a registry/manifest.

Do not add or depend on `mock`, `codex`, `command`, or `script` provider configs
unless the task is explicitly to revive that design. Current config
deserialization rejects those legacy kinds. Codex usage is provided as the
`codex-usage` remote provider example.

## Config And Secrets

Windows AppData config:

```text
%APPDATA%\QuotaBarWin\config.quotaBarWin.json
```

Portable config, selected when `quotabarwin.portable` exists beside the app
executable:

```text
<app-exe-dir>\config.quotaBarWin.json
```

Remote providers are cached under:

```text
%APPDATA%\QuotaBarWin\providers\remote\<provider-id>\
```

Secret placeholders:

- `${secret:NAME}` reads `<config-dir>\secrets\NAME.txt`, then env var `NAME`.
- `${env:NAME}` reads env var `NAME`.
- `${file:C:\path\secret.txt}` reads a local file and trims whitespace.

Never hard-code real API keys, tokens, cookies, account IDs, or proxy
credentials in code, docs, fixtures, or tests. Keep diagnostics redacted.

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

Build:

```powershell
npm run build
npm run tauri build
```

Tests:

```powershell
npm run test -- --run
cargo test --manifest-path src-tauri/Cargo.toml
```

Icons:

```powershell
npm run icons:check
```

E2E:

```powershell
npm run e2e
```

The E2E runner requires `tauri-driver` and a release build. Check
`e2e/tauri.e2e.mjs` before relying on it, because this area has historically
lagged behind provider schema changes.

## Testing Expectations

- Frontend changes should usually include or update Vitest tests next to the
  affected component or library.
- Rust backend changes should include focused unit tests in the touched module.
- Remote provider manifest/source changes must keep `checksums.source` in sync.
  `cargo test --manifest-path src-tauri/Cargo.toml` validates example manifests.
- If you touch config migration, test old schema inputs and rollback behavior.
- If you touch refresh behavior, test stale fallback, cache behavior, and
  single-provider refresh ordering.

## Coding Guidance

- Prefer existing patterns over new abstractions.
- Keep provider-specific parsing inside provider implementations or remote
  provider scripts; do not leak provider-specific fields into UI logic.
- Preserve stable `QuotaWindow.id` values. User customization depends on them.
- Use structured JSON parsing instead of string slicing when data is structured.
- Avoid shell command string concatenation for execution paths and arguments.
- On Windows, be careful with portable paths, AppData paths, and path separators.
- Keep UI rendering generic over `provider.windows`; do not hard-code `5h` and
  `weekly` as the only quota windows.

## Useful Local Files

- `examples/remote-providers/registry.json` installs the example remote
  providers as a registry.
- `fixtures/commands/` contains raw provider response fixtures used by example
  scripts and tests.
- `src-tauri/icons/source.svg` is the source icon; generated icon outputs should
  be updated with `npm run icons:generate`.

## Final Checks Before Handoff

Run the narrowest meaningful checks first, then broaden when risk warrants it.
For typical changes, report whether these passed:

```powershell
npm run build
npm run test -- --run
cargo test --manifest-path src-tauri/Cargo.toml
```

If a check cannot run locally, state why and name the residual risk.
