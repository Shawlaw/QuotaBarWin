<p align="center">
  <img src="src-tauri/icons/source.png" width="128" alt="QuotaBarWin icon">
</p>

<h1 align="center">QuotaBarWin</h1>

QuotaBarWin is a Windows-only desktop app for monitoring AI usage and quota windows across multiple services, with both a main window and a quick tray popup.

If you are on macOS, consider [CodexBar](https://github.com/steipete/CodexBar), a macOS menu bar app for monitoring AI usage.

Default documentation is Simplified Chinese: [README.md](README.md).

---

## Documentation

- [Simplified Chinese README](README.md)
- [Remote Provider Guide](docs/remote-provider-guide.en.md)
- [Local Integration API](docs/local-integration-api.en.md)
- [Agent / CLI Guide](docs/cli.en.md)
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
- Current version: **v1.4.1**
- Stack: Tauri 2, Rust 2021, React 19, TypeScript, Vite
- Current config schema version: **21**

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

## Quick Start

The QuotaBarWin release package does not include your account credentials and does not enable any service account by default. The app is configured with the QuotaBarWin-maintained remote Provider source, and Settings loads installable Providers from this registry:

[QuotaBarWin-maintained Provider registry](https://raw.githubusercontent.com/Shawlaw/QuotaBarWin/main/examples/remote-providers/registry.json)

**Security notice: only install and use Providers you trust. `builtin-js` can access only manifest-declared environment values, files, and network origins; external-runtime Providers run with the permissions of their own runtime.**

### 1. Install And Run

1. Download the Windows portable zip from GitHub Releases and extract it to any folder.
2. Run `QuotaBarWin.exe`. For the portable zip, config, logs, secrets, and Provider cache are stored beside the exe by default.
3. Project-maintained example Providers use the embedded `builtin-js` runtime, so installing and refreshing them does not require Node.js. A third-party Provider needs `node`, `python`, `pwsh`, `bash`, or another executable only when its manifest declares that runtime.

### 2. Choose A Project-Maintained Provider

The QuotaBarWin-maintained registry currently contains these manifests; the actual installable list is loaded from the registry:

| Manifest ID | Display name | What it monitors | Common credential / config |
|---|---|---|---|
| `kimi-coding` | Kimi Coding Usage | Kimi coding quota usage | `KIMI_API_KEY` |
| `bigmodel-coding-plan` | BigModel Coding Plan | Zhipu / BigModel coding plan quota | `BIGMODEL_API_KEY` |
| `codex-usage` | Codex Usage | ChatGPT / Codex 5h and weekly usage | Reads local Codex auth by default; can be overridden with `CODEX_ACCESS_TOKEN`; proxy notes are in section 5. |
| `deepseek-balance` | DeepSeek Balance | DeepSeek pay-as-you-go balance | `DEEPSEEK_API_KEY`, with optional reference total, low-balance threshold, and currency. |
| `time-flies` | 光阴似箭 (Time Flies) | Minutes remaining today, this week, this month, and this year | None; uses only the local timezone and current time. |

### 3. Install A Provider

**Before installing, confirm that the source is trusted. Provider scripts run on your machine, can read the tokens, API keys, cookies, account IDs, and other credentials configured for them, and can access the network.**

1. Open **Settings → Providers → Add Provider**.
2. The QuotaBarWin-maintained Provider source should load automatically. If the list is empty, open **Manage Source** and paste the registry URL above.
3. Click **Install** on the Provider you want. The app immediately opens that account's setup form; the new instance stays disabled and is excluded from scheduled refreshes until it is verified.
4. For multiple accounts on the same Provider, click **Add account** on the already-installed item.

### 4. Configure Credentials

After installation, enter the account name and required API Key in the form, then click **Save and test**. On success the Provider is enabled automatically and shows a short quota preview. On failure, the safely saved configuration is retained and the form offers a retry path.

The normal path does not require creating a `secrets` directory or txt file, entering `${secret:...}`, or editing raw environment variables. The app writes every account's secret automatically to:

```text
<config-dir>\secrets\providers\<provider-instance-id>\<parameter-name>.txt
```

The main config stores only the `${secret:providers/<provider-instance-id>/<parameter-name>}` reference, and existing secret values are never shown in the UI. This file is still local plaintext; it travels with the configuration directory in portable mode or backups, so protect that directory.

**Advanced compatibility:** existing `${secret:NAME}`, `${env:NAME}`, `${file:C:\path\secret.txt}`, literal values, and the raw **Environment variables** editor remain available for third-party Providers and automation. Upgrading never forces a migration.

### 5. Proxies

When Codex usage needs a proxy, set it in the Provider form's **Advanced settings**. Existing raw environment-variable configuration also remains valid, for example:

```text
HTTPS_PROXY=http://127.0.0.1:7890
```

The embedded runtime host handles Codex usage proxies: Provider `QBWIN_PROXY_URL` takes priority and the project-wide proxy is the fallback. When the project proxy is set to System, it reads the Windows process `HTTPS_PROXY` / `HTTP_PROXY` environment. Supported protocols are `socks5:`, `socks5h:`, `http:`, and `https:`. An installation-source proxy is used only to download registries, manifests, and scripts. For complete request, auth, and proxy notes, see [`examples/remote-providers/codex-usage/api.en.md`](examples/remote-providers/codex-usage/api.en.md).

Use **Settings → General → Network proxy → Test proxy** to test the current, even unsaved, proxy settings. The default target is the GitHub homepage. If an enterprise network or GitHub restriction makes that unsuitable, expand the advanced option and enter another HTTPS URL for this test only. The target is not saved, response content is discarded, and raw transport errors are not shown.

### 6. Multiple Accounts

For multiple accounts, click **Add account** on the installed Provider. Each new account gets its own Provider instance and managed secret directory, so its API Key cannot overwrite or be shared with another account. Advanced users can keep their own `${secret:...}` / `${env:...}` / `${file:...}` mappings.

### 7. Save And View

Click **Save and test** in the setup form. On success, click **Done** to return to Overview. A Provider left for later stays in the “Pending setup” state and does not participate in background refreshes. General settings and advanced raw configuration can still be saved with **Save** (or Ctrl+S).

---

## Local Integration API

Enable the HTTP API under **Settings → General → Local Integration API**. When enabled, it listens only on the loopback address by default: `http://127.0.0.1:41833`. Local automation, scripts, browser extensions, and other desktop tools can consume the normalized quota snapshot directly instead of exchanging files in the application directory.

- `GET /v1/health` reports service and snapshot availability.
- `GET /v1/snapshot` returns the latest normalized quota snapshot.
- `POST /v1/refresh` schedules a refresh; read the snapshot afterwards.

The default loopback listener is reachable only from the same computer and does not require authentication. In the same settings page, you can disable it, change its port, bind it to one or more active network interfaces with IPv4 or IPv6 addresses, or listen on every active interface; network modes keep `127.0.0.1` available without a token by default. When loopback remains selected, an empty external-interface selection is valid and runs as a local-only service. Before enabling non-loopback addresses, save either a manually entered or randomly generated Bearer token in the settings page. Network listener settings cannot be saved until a token exists; a saved token is shown in masked form and can be copied, replaced, or rotated. It is never written to the main configuration, logs, or API responses. This API exposes only read-only snapshots and refresh scheduling: it cannot change Providers, settings, or credentials.

See the [Local Integration API guide](docs/local-integration-api.en.md) for requests, responses, and security boundaries.

---

## Agent / CLI

The portable release also includes `QuotaBarWin.Cli.exe`. It reuses installed Providers, credentials, and configuration to give agents and scripts JSON-only quota queries and threshold decisions. It does not open a window or tray, or start another desktop-app instance.

For example, have an agent refresh Codex's five-hour window before a quota-intensive step:

```powershell
.\QuotaBarWin.Cli.exe check --provider codex-usage --window 5h --min-remaining-percent 20
```

Exit code `0` means work can continue, `10` means the quota is below the threshold and work should be deferred or switched, `11` means the data cannot be safely assessed (for example, a stale Provider), and `20` means refresh or configuration failed. The default is a live refresh; explicitly pass `--cached` to read only the latest disk snapshot.

See the [Agent / CLI Guide](docs/cli.en.md) for commands, JSON output, and agent orchestration examples. `resetAt` is a Provider-reported reset or suggested recheck time, not a guarantee that quota will be full then.

The CLI can also validate Provider configuration, manifest, source checksum, and runtime before installation or during troubleshooting: `QuotaBarWin.Cli.exe validate --provider codex-usage`. See the Provider validation section of the guide.

---

## Core Features

- Shows quota windows, remaining usage, reset times, status, and progress bars per Provider.
- Includes a local HTTP API and a JSON CLI for agents and scripts, with refresh, cached reads, and remaining-percent threshold decisions.
- Supports per-Provider manual refresh and global interval-based auto refresh.
- Includes Windows tray integration, hidden startup, single-instance behavior, and a resizable tray popup.
- Provides settings for refresh interval, display mode, low-quota warning threshold, language, light/dark theme, log level, and launch at startup. The theme follows Windows by default and can be fixed to light or dark.
- Supports AppData storage and portable mode. Portable mode keeps config, logs, secrets, and cached remote providers beside the exe.
- Supports Provider enablement, ordering, visible quota windows, and custom window labels.
- Installs Providers from remote registries/manifests with caching, SHA-256 verification, and update checks.
- Supports global and per-Provider proxies with HTTP and SOCKS5, including a pre-save global proxy connectivity test.
- Supports secret placeholders so real credentials do not need to be stored directly in config or logs.

---

## Provider Model

QuotaBarWin keeps one public data abstraction: **Provider**.

User-facing Providers are installed from remote registries/manifests. Provider scripts can use API calls, CLI tools, JSON parsing, text parsing, or other internals, but the UI only consumes normalized `AppSnapshot`, `ProviderSnapshot`, and `QuotaWindow` data.

Provider configuration type:

| Kind | Source | Notes |
|---|---|---|
| `remote` | Cached Provider script | Installed from a registry/manifest. Official Providers use embedded `builtin-js`; `node`, `python`, `pwsh`, `bash`, and absolute executable paths remain supported. |

---

## Runtime And Software Dependencies

QuotaBarWin has three separate dependency surfaces: the app itself, remote Providers, and the development environment.

| Scenario | Required software | Notes |
|---|---|---|
| Running the released `QuotaBarWin.exe` / `QuotaBarWin.Cli.exe` | Windows; the GUI needs Microsoft Edge WebView2 Runtime | The portable zip contains the desktop app and CLI. The CLI does not require WebView2. Neither requires users to install Node.js, npm, Rust, or the Tauri CLI. |
| Installing / running remote Providers | No extra software for `builtin-js`; matching software for any other runtime | `builtin-js` uses embedded QuickJS, and every project-maintained Provider currently uses it. External runtimes are resolved from `PATH` or an absolute path and validated during install / update; a Provider declaring `"runtime": "node"` needs a working `node`. |
| Developing, testing, or building this repo | Node.js 22+, npm, Rust stable / Cargo, Windows MSVC build tools | `package.json` requires `node >=22`, and the release workflow also uses Node 22. `npm run tauri ...` and `cargo test ...` need the Rust toolchain; full E2E also needs `tauri-driver`. |

External-runtime Provider scripts may also call additional CLIs or read local credential files. Those are not universal QuotaBarWin dependencies and should be documented by the individual Provider. `builtin-js` provides neither subprocesses nor arbitrary file access; see the [remote Provider guide](docs/remote-provider-guide.en.md#embedded-javascript-runtime-builtin-js).

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

In portable mode, the remote provider cache is stored beside the exe with the rest of the portable data. Switching storage modes or starting an updated app migrates existing Provider caches; a migration or config-write failure rolls back to avoid a partial move.

Secret placeholders:

- `${secret:NAME}`: reads `<config-dir>\secrets\NAME.txt`, then falls back to the environment variable `NAME`.
- `${env:NAME}`: reads the environment variable `NAME`.
- `${file:C:\path\secret.txt}`: reads a local file and trims surrounding whitespace.

For multiple accounts on the same provider, set each account instance's
Environment variables to map the script's env var to a different secret, such as
`KIMI_API_KEY=${secret:KIMI_WORK_API_KEY}`, which reads
`<config-dir>\secrets\KIMI_WORK_API_KEY.txt`. See
[`docs/remote-provider-guide.en.md`](docs/remote-provider-guide.en.md#local-config-and-secrets).

Do not put real API keys, tokens, cookies, account IDs, or proxy credentials in code, docs, fixtures, or tests.

---

## Development

Before starting, make sure the development prerequisites above are installed.

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

Preview the application-update UI (it does not make a network request, download anything, or modify the update cache):

```powershell
$env:QBWIN_DEMO_APP_UPDATE="1"
npm run tauri -- build --features update-preview --no-bundle
& .\src-tauri\target\release\quotabarwin.exe
```

Open the main window or tray popup after startup. About 700ms later, one simulated update notice lets you try the entrance animation, **Go to update**, and **Later**. Set the environment variable to `manual` instead to start without a notice; **Settings → Application update → Check for updates** will then simulate the update discovery, which is useful for verifying that the main window and tray popup synchronize a manual result. Normal Release builds do not include this preview capability; only an internal build made explicitly with the `update-preview` feature reads this environment variable.

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

Note: E2E is currently an optional maintainer check and does not replace the regular build and unit test checks above. Quit all running QuotaBarWin instances first; Tauri single-instance behavior redirects startup to the existing process. The script builds the release app and writes temporary portable config plus a local remote Provider cache. Set `QBWIN_E2E_KEEP_ARTIFACTS=1` when debugging to keep those temporary files.

---

## Build And Release

Local release exe build:

```powershell
npm run tauri -- build --no-bundle
```

Windows releases are produced by `.github/workflows/release.yml`. The current release workflow builds `QuotaBarWin.exe`, `QuotaBarWin.Cli.exe`, and `QuotaBarWin.Updater.exe`, then packages them as a portable zip with this filename format:

```text
QuotaBarWin_<version>_windows_x64_portable_<commit>.zip
```

The zip includes `quotabarwin.portable`, so extracted releases use portable config beside the executable by default. On `v*` tags, the zip is uploaded as a GitHub Release asset and the matching `CHANGELOG.md` section becomes the Release notes; manual workflow runs keep it as an Actions artifact.

### Application-update release setup

Application updates use DeskFoundry's `desktop-updater`. The client reads signed `updates/stable.json` and `.sig` data from GitHub Raw and downloads the ZIP from GitHub Releases. It replaces only allow-listed executables; the `quotabarwin.portable` marker, configuration, logs, secrets, Provider caches, and user files are retained.

Before enabling the first production update, maintainers must create a distinct QuotaBarWin Ed25519 key as described in the [DeskFoundry portable-update guide](https://github.com/Shawlaw/DeskFoundry/blob/main/docs/portable-update-guide.md). Store the private key as repository Actions secret `DESKTOP_UPDATE_PRIVATE_KEY`, and set its public key as repository variable `QUOTABARWIN_UPDATE_PUBLIC_KEY`. The Release workflow compiles that public key into the client, packages `QuotaBarWin.Updater.exe`, and uses the fixed `DeskFoundry@v0.1.4` Action to sign and commit the Raw update pointer; before doing so, the Action verifies that the secret private key matches this public key. `desktop-update.toml` defines only the ZIP replacement allow-list.

---

## Install And Uninstall

Install: download the portable zip from GitHub Releases, extract it to any folder, and run `QuotaBarWin.exe`.

The first release containing the updater must still be installed manually. Later releases can be checked from **Settings → Application update** and installed with **Download and restart to update**.

**Settings → Application update** enables automatic checks by default. Once after 08:00 local time each day, the first opening of either the main window or tray popup checks in the background; if a check temporarily fails, the app retries later. When a new version is found, whether automatically or manually, both windows show an update notice; **Later** closes it and silences that version. **Go to update** opens Settings and focuses the **Application update** section so the user can continue with **Download and restart to update**. Download always uses the signed candidate represented by the notice, rather than a version that may be published later; manually checking again refreshes that candidate. Automatic checks never download, install, or restart the app. Users can turn this feature off at any time in Settings.

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
