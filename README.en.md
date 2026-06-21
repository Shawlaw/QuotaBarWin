# QuotaBarWin

Default documentation is Simplified Chinese: [`README.md`](README.md).

Windows-first AI usage and quota monitor built with Tauri 2, Rust, React, and
Vite.

The app keeps quota data behind one public abstraction: **Provider**.
User-configurable providers are installed from remote registries/manifests. A
provider script can use CLI calls, API requests, JSON parsing, or text parsing
internally, but the frontend only renders normalized `ProviderSnapshot` /
`AppSnapshot` data.

## Current Features

- Compact desktop overview with provider cards, quota windows, progress bars,
  global status, and per-provider refresh.
- Windows tray integration, resizable tray popup window, hidden startup mode,
  and single-instance behavior. Double-click the tray popup title area to reset
  its default size.
- Configurable refresh interval, display mode, low-quota warning threshold,
  language, log level, launch-at-startup, and provider ordering.
- AppData and portable config storage. Portable mode is enabled by creating
  `quotabarwin.portable` next to the app executable.
- Provider window customization with `visibleWindowIds` and
  `windowLabelOverrides`.
- Remote provider registries that install cached provider scripts from
  manifests with optional SHA-256 verification and update checks. Settings
  shows provider version, install time, update time, and last check time.
- Global and per-provider proxy support for HTTP and SOCKS5 URLs.
- Secret placeholders for provider config: `${secret:NAME}`, `${env:NAME}`,
  and `${file:C:\path\secret.txt}`.
- Redacted diagnostics zip export.

## Provider Model

The current implementation supports one user-configurable provider kind:

| Kind | Source | Notes |
|---|---|---|
| `remote` | Cached external script | Installed from a registry/manifest and executed with its declared runtime, such as `node`, `python`, `pwsh`, `bash`, or an absolute executable path. |

Older `mock` / `codex` / `command` / `script` provider designs remain in
historical specs, but they are not accepted by the current config schema. Codex
usage is available through the `codex-usage` remote provider example.

Remote provider authoring is documented in
[`docs/remote-provider-guide.md`](docs/remote-provider-guide.md). Complete
examples live in [`examples/remote-providers/`](examples/remote-providers/).

## Config Storage

Current schema version: `13`.

Windows AppData config:

```text
%APPDATA%\QuotaBarWin\config.quotaBarWin.json
```

Portable config, when `quotabarwin.portable` exists beside the executable:

```text
<app-exe-dir>\config.quotaBarWin.json
```

Remote provider scripts are cached under the app data directory:

```text
%APPDATA%\QuotaBarWin\providers\remote\<provider-id>\
```

Named secrets are read from `<config-dir>\secrets\NAME.txt` first and then fall
back to the process environment variable `NAME`.

## Development

Install dependencies:

```powershell
npm install
```

Run the browser preview:

```powershell
npm run dev
```

Run the Tauri desktop app:

```powershell
npm run tauri dev
```

Build frontend assets:

```powershell
npm run build
```

Run tests:

```powershell
npm run test -- --run
cargo test --manifest-path src-tauri/Cargo.toml
```

Icon checks:

```powershell
npm run icons:check
```

The E2E runner uses `tauri-driver` and WebDriverIO:

```powershell
npm run e2e
```

Install `tauri-driver` first if needed:

```powershell
cargo install tauri-driver --locked
```

## Build Artifacts

Windows release builds are produced by `.github/workflows/release.yml`. The
workflow builds the Tauri installer artifacts and also creates a portable zip
containing `QuotaBarWin.exe`.

Local installer build:

```powershell
npm run tauri build
```

Tauri bundling is configured for MSI and NSIS targets in
[`src-tauri/tauri.conf.json`](src-tauri/tauri.conf.json).

## Install

Download the Windows installer from the release artifacts and run it. The
portable zip can also be extracted to any folder and launched directly with
`QuotaBarWin.exe`.

## Uninstall

If installed with the Windows installer, uninstall from Windows Settings >
Apps > Installed apps > QuotaBarWin. If using the portable zip, quit
QuotaBarWin from the tray menu and delete the extracted folder.

Remove `%APPDATA%\QuotaBarWin` if you also want to delete local settings,
diagnostics, logs, secrets, and cached remote providers.
