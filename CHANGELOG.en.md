# Changelog

This file records user-visible changes in QuotaBarWin. Simplified Chinese counterpart: [CHANGELOG.md](CHANGELOG.md).

## [1.6.2] - 2026-10-08

### Added

- Notifications can now be turned off per provider: expand a provider under Settings → Providers and toggle "Send notifications for this provider's events"; every provider participates by default. Turning it off only affects delivery — that provider's events are still recorded in the local event history, they just no longer trigger Windows notifications or webhooks. A provider's manifest may carry a different initial default at install time (for example, "光阴似箭 (Time Flies)" opts out by default); afterwards the choice is entirely the user's and updates never overwrite it.

## [1.6.1] - 2026-10-07

### Added

- Windows system notifications now append the local time of the event (`MM-DD HH:mm`) to the end of the notification body.
- Added a "Quota expiry time changed" event: a window that consumed no quota at all in the previous cycle is now recorded as this event when its cycle rolls over (including the expiry times before and after the change), instead of as "Quota reset"; the event is excluded from notifications by default and can be enabled individually under Settings → Notifications.
- Webhook notifications support multiple endpoints (each with its own URL, message template, and timeout). Events are delivered to every enabled endpoint, and one endpoint failing does not affect the others. Each endpoint can define a custom message template: when set, each selected event renders and sends its own request, with placeholders such as `{{message}}` (the notification body including the event time), `{{eventType}}`, `{{providerName}}`, `{{windowLabel}}`, `{{occurredAt}}`, and `{{eventJson}}`; a rendered body that parses as valid JSON is sent as JSON, otherwise as plain text; leaving the template empty keeps the previous JSON batch format. Next to the template input in Settings, a new "Open the webhook template guide" entry opens the built-in offline bilingual guide (with the full placeholder table, the default JSON batch structure, and template examples for DingTalk, Feishu, and similar services).

### Changed

- All in-app confirmation dialogs (reset config, clear event history, migrate Provider source, etc.) now use the app's own styling instead of system dialogs with browser chrome; the notice shown when a duplicate launch is rejected is now an in-window toast as well.
- Configuration backup files (from migration, reset, and corruption recovery) are now collected under the `bak` subdirectory of the configuration directory instead of sitting flat next to the config; on first run the app also sweeps leftover backups from older versions into `bak` automatically.

### Fixed

- Fixed quota-reset event detection: a window with no usage at all (staying at 0% for a long time) was misrecorded as "Quota reset" and repeatedly notified at every cycle rollover; when a provider temporarily stopped reporting the reset time right after a window rolled over (with usage already zeroed), that reset was missed; a window that rolled over earlier than previously announced (reset time moved earlier and quota emptied) was misjudged as "Unexpected quota recovery" instead of "Quota reset". All of these scenarios are now classified correctly, and "Unexpected quota recovery" is reserved for quota reappearing mid-cycle while the reset time is completely unchanged.
- Fixed the Settings page still prompting for a manual save after "Reset config": a reset now takes effect immediately and syncs the Settings page state, so no extra save click is needed.
- Fixed the tray popup header (title, buttons, status line) sitting too close to the left and right edges of the popup.

## [1.6.0] - 2026-10-06

### Added

- Added an in-app event history page (the "Events" tab at the top of the main window): it records events such as application started, upgrade completed, quota reset, unexpected quota recovery, quota exhausted, quota low, and Provider refresh failures and recoveries, with category filters, refresh, and clear actions; startup events record the version at that time (including the short commit hash). Events are stored only in the local configuration directory, capped at 200 entries.
- Added quota event notifications, enableable under Settings → Notifications: two channels are supported, Windows system notifications (toast) and webhooks, and the event types to notify can be selected individually. Windows notifications now use the QuotaBarWin branded icon; when Windows system notifications are turned off, the app automatically clears the local notification registration (AppUserModelID), so no manual cleanup is needed. "Send test notification" uses the values currently entered in the form, so there is no need to save first.
- Webhook notifications deliver the selected events as a JSON POST to the configured address; the URL supports `${secret:NAME}`, `${env:NAME}`, and `${file:...}` placeholders so token-bearing addresses are not written into the config in plaintext; requests follow the app's global proxy settings, and delivery failures are recorded only in redacted local logs.
- Notifications are evaluated after each refresh confirms a change, so they arrive on the next refresh after the change happens (default interval 300 seconds, adjustable in Settings).
- Added an "Open guide" entry at the bottom of the Settings page's left navigation, which opens the built-in remote Provider usage guide at any time.

### Changed

- The Settings page is reorganized into five categories (Providers / General / Notifications / Application update / Advanced), switched via the left navigation: the network proxy, local integration API, secret security, configuration storage, and logging settings moved into Advanced; the low-quota warning threshold is now edited on the Notifications page, while General shows a summary with a one-click jump. The save flow for all settings (bottom save bar / Ctrl+S) and the config file format are unchanged, so existing configuration needs no reconfiguration.
- The network proxy no longer requires choosing between HTTP and SOCKS5 manually: after selecting "Custom proxy", the protocol prefix of the address (`http://`, `socks5://`, etc.) is detected automatically, and existing configuration remains fully compatible.
- Closing the main window with unsaved settings changes now asks whether to save or discard before hiding to the tray, so changes are not lost by accident.
- Copying the config file path or the portable marker path in Configuration storage now shows a "Path copied to the clipboard" notice instead of copying silently.
- The built-in usage guide now uses the "Provider" naming throughout and organizes its instructions and examples around the built-in QuickJS runtime (`builtin-js`) as the recommended path.

### Fixed

- Fixed Provider card shadows being clipped on the left and right sides in the tray popup, and restored the spacing between the header area and the popup edges.

## [1.5.2] - 2026-10-05

### Added

- The Secret security encryption migration now also covers manually created txt secret files under the `secrets` directory (including the startup migration prompt and the plaintext count): once encrypted, these files still resolve through the same `${secret:NAME}` placeholder, but they can no longer be edited directly in a text editor; disabling encryption restores them to plaintext.
- When encryption is enabled but plaintext secret files are still detected (for example, encryption was enabled on an older version whose migration did not cover hand-managed files at the time), Secret security now shows an "Encrypt remaining plaintext secrets" button that completes the migration in one click, without toggling encryption off and on again.

### Fixed

- Right-clicking inside the app no longer opens the WebView's built-in web menu (entries such as "Refresh" reload the interface, which conflicts with how the app behaves); Cut/Copy/Paste from right-clicking input fields still work, and shortcuts such as Ctrl+C on selected text are unaffected.

## [1.5.1] - 2026-10-05

### Fixed

- When a Provider bumps only its version number without changing the script (for example, an official Provider moving from 1.1.0 to 1.2.0), "Check Updates" now correctly reports an available update and can apply it, instead of showing the old version and claiming it is up to date.
- After copying or moving the portable app directory, the Provider cache no longer follows the old directory: the cache is always resolved beside the current app directory, so deleting or updating Providers in the copied instance no longer removes or overwrites the cache in the original directory.
- When the Provider cache directory is missing or corrupted (for example, accidentally deleted), refreshing now repairs it automatically by re-downloading from the configured manifest URL, instead of requiring the Provider to be added again manually to restore readings.
- The CLI's `validate --provider` likewise validates the Provider cache beside the active configuration directory; a missing cache now produces an actionable hint (refresh once in the app to repair it automatically) instead of relying on stale paths recorded in the config.

## [1.5.0] - 2026-10-05

### Added

- Provider secrets support local encrypted storage: secrets are encrypted with Windows data protection (DPAPI), and the ciphertext can only be decrypted by the current Windows account. Newly saved secrets are encrypted by default; users upgrading from older versions see a one-time confirmation prompt on first launch asking whether to migrate existing plaintext secrets to encrypted storage, and encryption can also be enabled or disabled later in the Settings page's Secret security section.

### Fixed

- After an update check fails temporarily (for example, the update server is briefly unreachable), a previously detected update can still be downloaded normally, instead of incorrectly demanding a fresh check first.
- An already downloaded update package is no longer discarded because a later update check fails, so it does not need to be downloaded again.
- After restarting the app, a previously detected update can still be downloaded directly: the signed manifest is re-verified offline and the downloaded package is reused, even when the update server is temporarily unreachable.
- Downloading a large update package over a slow connection no longer breaks on a fixed 30-second total timeout; the download timeout now scales with the package size.
- Provider refresh network problems no longer show as a single cryptic HTTP error: a timed-out request reports the timeout in seconds and the domain that did not respond, a failed connection reports the specific reason, and when a proxy is configured the message clearly notes that traffic goes through the proxy, making it easier to tell proxy problems from target API problems.
- URLs in logs and the UI now keep the domain name, making it easier to see which API failed; paths, query strings, and credentials remain hidden.
- When adding a Provider, the "View guide" link below the secret input now opens the help documentation in the system browser correctly.

## [1.4.2] - 2026-09-19

### Fixed

- The tray popup no longer retains the previous keyboard focus when reopened, avoiding stray focus rings and Enter accidentally triggering the previously selected button.

## [1.4.1] - 2026-08-24

### Fixed

- A temporarily failed application update check is retried automatically, so later checks on the same day are not skipped.
- When a manual check finds a new version, the main window and tray popup show the update notice promptly.
- The main window and tray popup now keep a minimum size at which they still render correctly, preventing broken layouts when scaled too small.

## [1.4.0] - 2026-08-24

### Added

- Added theme selection in Settings: follow the Windows system theme, or pin the app to the light or dark theme.
- The main window and tray popup both apply the selected theme consistently.

### Fixed

- After a newly added Provider completes "Save and test", it now refreshes immediately and appears in the overview, without waiting for the background scheduled refresh.

## [1.3.0] - 2026-08-20

### Added

- Added a local integration HTTP API that on-device automation, scripts, browser extensions, and other desktop tools can use to read quota snapshots or trigger refreshes directly.
- Added "Local Integration API" in Settings: start or stop the service, change the port, and choose to listen on the loopback address, multiple active IPv4/IPv6 adapters, or all active adapters.

### Security

- Disabled by default; when enabled, it listens only on `127.0.0.1`. Network listeners can keep the token-free loopback address at the same time; enabling external adapters requires saving a manually entered or randomly generated Bearer token first, and network listener settings cannot be saved without a stored token; a saved token is displayed masked and can be copied, replaced, or regenerated.
- API responses never include Provider metadata, diagnostics, or sensitive error content, and the API provides no way to write configuration, Providers, or credentials.

## [1.2.3] - 2026-08-15

### Changed

- "Automatically check for application updates" is enabled by default; users can still turn it off in Settings at any time.
- Improved temporary file cleanup after an application update.

## [1.2.2] - 2026-08-15

### Added

- Added an "Automatically check for application updates" toggle in Settings. Once after 08:00 local time each day, the first opening of the main window or the tray popup checks for a new version.
- When an update is found, both the main window and the tray popup show an upgrade notice with a direct path to the update.

### Changed

- Improved the application update flow and the browsing experience of the overview and Settings pages.

## [1.2.1] - 2026-08-03

### Fixed

- When the main window regains focus, or when switching back from Settings to the overview, the latest native quota cache is read immediately and the interface refreshes.
- The background scheduled refresh keeps running independently and continues updating the cache while the main window is hidden or not in the foreground.

## [1.2.0] - 2026-07-31

### Added

- After a Provider is installed, the app goes straight into a structured configuration form; once the account information is filled in and "Save and test" succeeds, the Provider is enabled automatically.
- The app automatically creates an isolated local secret file for each Provider instance and parameter, and the main config keeps only the `${secret:providers/...}` reference.
- Provider settings gained Pending setup, Unverified, Ready, and Needs attention states, plus actionable setup and repair entries.
- Official Provider manifests gained API Key help links and advanced field markers.
- The global network proxy gained an availability check: it visits the GitHub homepage by default, and a custom HTTPS check URL can be specified for the test only.

### Changed

- Newly installed Providers stay disabled until they pass verification and do not participate in scheduled refreshes; a failed test keeps the safely saved configuration and leaves the Provider disabled.
- The normal configuration path no longer requires creating a secret directory, typing placeholders, or editing raw environment variables; raw envVars and legacy placeholders remain available for advanced users and third-party Providers.
- Deleting a Provider now offers, by default, a confirmation option to clean up that instance's app-managed secret files; external files, environment variables, and hand-named secrets are never deleted.

### Security

- Saved secrets are never returned to the frontend, nor written into the main config, logs, errors, diagnostics, test snapshots, or copied diagnostics.
- `${secret:...}`, `${env:...}`, `${file:...}`, legacy plaintext config, and the existing Provider runner/resolver all remain compatible; this version introduced no system credential store, DPAPI, custom encryption, or cross-device sync.
- The proxy check does not save response content and does not return raw transport errors or proxy credentials to the frontend.

## [1.1.1] - 2026-07-27

### Fixed

- Fixed "Apply Update" pushing "More" onto a second line and making the Provider card grow abnormally once a Provider update was detected.
- Fixed the previous round's applicable-update button lingering and contradicting the failure notice when a later Provider update check fails.

## [1.1.0] - 2026-07-27

### Added

- Built-in QuickJS Provider runtime: official Providers no longer depend on the user's local Node.js; Provider authors can use the sandboxed `qb` API to read declared environment variables, files, and HTTP endpoints.
- Added complete `builtin-js` Provider author documentation, CLI instructions, and examples, covering permission declarations, the execution model, the output protocol, and the support boundary.

### Changed

- All maintained official Providers (Kimi, BigModel Coding Plan, DeepSeek, Codex Usage, 光阴似箭 (Time Flies)) have migrated to `builtin-js`.
- Provider runtime proxies follow the app's explicit proxy policy; unless "System proxy" is selected, system HTTP(S) proxy environment variables are never read implicitly.
- Provider update checks now follow the enabled source registry; results are shown directly on the corresponding Provider row, and updates can be applied one by one or all at once.

### Fixed

- Official `builtin-js` Providers now use manifest schema 2 and `minAppVersion`. Older app versions reject the update before downloading the new script and keep the still-working legacy Node Provider, preventing an automatic update from leaving the Provider unusable.
- Fixed the Codex Provider's permission check referring to a path inconsistent with the actual `~/.codex/auth.json` file, and simplified the built-in JS error messages.
- Fixed an inconsistency where the "Add Provider" page already showed the new source version while installed Providers still checked updates against the old manifest; applying the update switches to the same verified source.

### Security

- `builtin-js` restricts environment variables, files, and network access to the manifest-declared permissions, and reserves the `QBWIN_*` host variables for internal platform use.

## [1.0.7] - 2026-07-23

### Fixed

- Fixed the tray popup restoring the window's outer frame size as the content size every time it was reopened from the tray when no manual size had been saved, causing the width and height to keep growing.

## [1.0.6] - 2026-07-22

### Fixed

- The GitHub entry in the main interface and the "Release notes" link in the update area now open in the Windows default browser, avoiding unresponsive external links in the Tauri WebView.
- The update settings page now shows the current app version on load, without requiring an update check first.

## [1.0.5] - 2026-07-22

### Changed

- Updated the app icon.

### Fixed

- Fixed the popup not always positioning itself based on the current tray click location when opened from the tray.

## [1.0.4] - 2026-07-21

### Added

- Added the portable app update foundation powered by DeskFoundry `desktop-updater`: a signed GitHub Raw update manifest, GitHub Release ZIP downloads, SHA-256 verification, file replacement by a standalone helper, and startup-confirmed rollback.
- Added an "Application update" check entry in Settings; once a new version is found, "Download and restart to update" can be run.

### Security

- App updates accept only Ed25519-signed manifests, and the ZIP may only overwrite executables in the release package whitelist; the portable marker, config, logs, secrets, Provider cache, and user files are all preserved.

## [1.0.3] - 2026-07-15

### Changed

- The install-source proxy now applies only to downloading registries, manifests, and Provider scripts; the Provider runtime proxy now prefers its `QBWIN_PROXY_URL` environment variable and falls back to the app-wide proxy only when it is unset.
- The config schema moved to 16; the upgrade clears the legacy Provider `proxyUrl` field left by older versions so it can no longer override the Provider's own runtime proxy configuration.

### Fixed

- Fixed an older network request returning after a newer one while the Provider directory was loading, which overwrote the successful list with an "install source has no data" error.

## [1.0.2] - 2026-07-14

### Added

- Added the 光阴似箭 (Time Flies) Provider: shows the remaining time in minutes for today, this week (Monday/Sunday start), this month, and this year.
- Settings can now control per Provider whether it appears in the tray popup; the default is to show it.

### Changed

- The main window and tray popup moved detailed quota numbers to hover, making the interface simpler by default; reset times remain visible.

### Fixed

- Fixed portable mode still writing the remote Provider cache to AppData. The cache now lives beside the active configuration directory; switching modes and the first launch of the new version migrate the old cache, and a failed migration rolls back to avoid a half-migrated state.

## [1.0.1] - 2026-07-14

### Fixed

- Saving a remote Provider source now keeps the Settings page, returns to "Add Provider", and immediately refreshes the directory using the latest proxy.
- The source proxy input hint now states clearly that HTTP and SOCKS5 URLs are supported.
- Fixed the tray popup auto-height counting blank space as content height when there is a single quota window, and prevented the auto-height from retriggering in a loop while the user resizes the window.

## [1.0.0] - Initial release

### Added

- Windows-first Tauri 2 desktop app with a main window and a tray popup.
- Remote Provider model with external Provider scripts installed from a registry / manifest.
- Provider enable/disable, ordering, window visibility control, window label overrides, manual refresh, and automatic refresh.
- AppData and portable configuration modes, covering logs, secrets, the remote Provider cache, and the snapshot disk cache.
- Global and per-Provider network proxies supporting HTTP and SOCKS5.
- Secret placeholders: `${secret:NAME}`, `${env:NAME}`, `${file:C:\path\secret.txt}`.
- Diagnostics export, structured redacted logs, and log rotation.
- Windows portable zip release workflow.

### Security

- Remote Provider stdout accepts only the final JSON payload; stderr goes into redacted logs and diagnostics.
- The snapshot disk cache strips Provider `metadata`.
- The Tauri WebView enables a baseline CSP.
- Added a security disclosure process, dependency review, Dependabot, and npm audit checks ahead of open-sourcing.
