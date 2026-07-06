<p align="center">
  <img src="src-tauri/icons/source.png" width="128" alt="QuotaBarWin icon">
</p>

<h1 align="center">QuotaBarWin</h1>

QuotaBarWin is a Windows-first desktop app for monitoring AI usage and quota windows across multiple services, with both a main window and a quick tray popup.

Default documentation is Simplified Chinese: [README.md](README.md).

---

## Documentation

- [Simplified Chinese README](README.md)
- [Remote Provider Guide](docs/remote-provider-guide.en.md)
- [Remote Provider Examples](examples/remote-providers/)
- [Remote Provider registry example](examples/remote-providers/registry.json)
- [Contribution Guide](CONTRIBUTING.md)
- [Security Policy](SECURITY.md)
- [Code of Conduct](CODE_OF_CONDUCT.md)
- [Open Source Readiness Audit](docs/open-source-checklist.md)

---

## Release Shape

- Platform: **Windows**
- Distribution: **portable zip + single exe**
- Current version: **v1.0.0**
- Stack: Tauri 2, Rust 2021, React 19, TypeScript, Vite
- Current config schema version: **14**

---

## Screenshots

The main window includes an overview page and a settings page. The tray popup is optimized for quick quota checks.

<p align="center">
  <img src="assets/screenshots/readme-zh-overview.png" alt="QuotaBarWin overview screenshot" width="49%">
  <img src="assets/screenshots/readme-zh-settings.png" alt="QuotaBarWin settings screenshot" width="49%">
</p>

<p align="center">
  <img src="assets/screenshots/readme-zh-tray-popup.png" alt="QuotaBarWin tray popup screenshot" width="42%">
</p>

---

## Core Features

- Shows quota windows, remaining usage, reset times, status, and progress bars per Provider.
- Supports per-Provider manual refresh and global interval-based auto refresh.
- Includes Windows tray integration, hidden startup, single-instance behavior, and a resizable tray popup.
- Provides settings for refresh interval, display mode, low-quota warning threshold, language, log level, and launch at startup.
- Supports AppData storage and portable mode. Portable mode keeps config, logs, secrets, and cached remote providers beside the exe.
- Supports Provider enablement, ordering, visible quota windows, and custom window labels.
- Installs Providers from remote registries/manifests with caching, SHA-256 verification, and update checks.
- Supports global and per-Provider proxies with HTTP and SOCKS5.
- Supports secret placeholders so real credentials do not need to be stored directly in config or logs.

---

## Provider Model

QuotaBarWin keeps one public data abstraction: **Provider**.

User-facing Providers are installed from remote registries/manifests. Provider scripts can use API calls, CLI tools, JSON parsing, text parsing, or other internals, but the UI only consumes normalized `AppSnapshot`, `ProviderSnapshot`, and `QuotaWindow` data.

Provider configuration type:

| Kind | Source | Notes |
|---|---|---|
| `remote` | Cached external script | Installed from a registry/manifest and executed with its declared runtime, such as `node`, `python`, `pwsh`, `bash`, or an absolute executable path. |

---

## Config And Privacy

Windows AppData config:

```text
%APPDATA%\QuotaBarWin\config.quotaBarWin.json
```

Portable config, when portable mode is enabled:

```text
<app-exe-dir>\config.quotaBarWin.json
```

Remote provider script cache:

```text
%APPDATA%\QuotaBarWin\providers\remote\<provider-id>\
```

In portable mode, the remote provider cache is stored beside the exe with the rest of the portable data.

Secret placeholders:

- `${secret:NAME}`: reads `<config-dir>\secrets\NAME.txt`, then falls back to the environment variable `NAME`.
- `${env:NAME}`: reads the environment variable `NAME`.
- `${file:C:\path\secret.txt}`: reads a local file and trims surrounding whitespace.

Do not put real API keys, tokens, cookies, account IDs, or proxy credentials in code, docs, fixtures, or tests.

---

## First Run

1. Download the Windows portable zip from GitHub Releases.
2. Extract it to any folder and run `QuotaBarWin.exe`.
3. Open settings and adjust refresh interval, language, proxy, launch at startup, and log level as needed.
4. Add or update remote Providers in the Provider section.
5. Return to the overview page or tray popup to check quota status.

---

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

Check icons:

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

---

## Build And Release

Local release exe build:

```powershell
npm run tauri -- build --no-bundle
```

Windows releases are produced by `.github/workflows/release.yml`. The release workflow builds `QuotaBarWin.exe` and packages it as a portable zip with this filename format:

```text
QuotaBarWin_<version>_windows_x64_portable_<commit>.zip
```

The zip includes `quotabarwin.portable`, so extracted releases use portable config beside the executable by default. On `v*` tags, the zip is uploaded as a GitHub Release asset; manual workflow runs keep it as an Actions artifact.

---

## Install And Uninstall

Install: download the portable zip from GitHub Releases, extract it to any folder, and run `QuotaBarWin.exe`.

Uninstall: quit QuotaBarWin from the tray menu, then delete the extracted folder.

The portable zip stores settings, logs, secrets, and cached remote Providers in the extracted folder by default. If you previously used AppData mode, you can also remove `%APPDATA%\QuotaBarWin`.

---

## Troubleshooting

- If quota data stops updating, first check whether the Provider is enabled in settings, then try a per-Provider refresh or global refresh.
- If remote Provider installation or update fails, check the registry/manifest URL, network proxy, and SHA-256 checksum.
- If a Provider needs credentials, prefer secret placeholders instead of writing secrets directly into config.
- When reporting an issue, include the relevant time range from the app log and manually remove sensitive information.

---

## Open Source

- License: [MIT](LICENSE)
- Contribution flow: [CONTRIBUTING.md](CONTRIBUTING.md)
- Security vulnerability reporting: [SECURITY.md](SECURITY.md)
- Code of Conduct: [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md)

Before opening an issue or pull request, make sure it does not include real API keys, tokens, cookies, account IDs, sensitive log excerpts, or proxy credentials.
