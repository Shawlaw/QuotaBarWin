# QuotaBarWin

WindowsFirst AI usage/quota monitor.

Architecture principle:

Provider is the only public abstraction that supplies quota data to the app.
A provider may internally execute commands, call curl/CLI/scripts, parse JSON,
parse text, or use native logic, but the UI and app runtime only depend on
normalized ProviderSnapshot / AppSnapshot data.

## Development

```powershell
npm install
npm run build
npm run test -- --run
cargo test --manifest-path src-tauri/Cargo.toml
```

## Build Artifacts

Windows release builds are produced by `.github/workflows/release.yml`. The
workflow builds the Tauri installer artifacts and also creates a portable zip
containing `QuotaBarWin.exe`.

Local installer build:

```powershell
npm run tauri build
```

## Install

Download the Windows installer from the release artifacts and run it. The
portable zip can also be extracted to any folder and launched directly with
`QuotaBarWin.exe`.

## Uninstall

If installed with the Windows installer, uninstall from Windows Settings >
Apps > Installed apps > QuotaBarWin. If using the portable zip, quit
QuotaBarWin from the tray menu and delete the extracted folder.

User configuration is stored under `%APPDATA%\QuotaBarWin\config.json` on
Windows. Remove that folder if you also want to delete local settings,
diagnostics, and logs.
