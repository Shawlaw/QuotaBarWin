# Local Integration API

Starting with QuotaBarWin 1.3.0, the local HTTP API lets local scripts, automation, browser extensions, and other desktop applications read normalized quota data. It is not a Provider contract and does not replace the [Agent / CLI](cli.en.md) threshold-decision workflow.

## Enablement and listening scope

It is disabled by default. When enabled, it listens only at:

```text
http://127.0.0.1:41833
```

Use **Settings → General → Local Integration API** to disable it or change the port. Valid ports are `1` through `65535`.

To let trusted devices on a LAN connect, select one or more active network interfaces; the server listens on every active IPv4 and IPv6 address for each selected interface. You can also select all active network interfaces to listen on every current active IP address. Network modes keep `127.0.0.1` listening without a token by default, and the settings page can disable that additional listener. When loopback remains selected, an empty external-interface selection can be saved and runs as a local-only service. After a network change, save the settings again or restart the app to rebind addresses. Before enabling non-loopback addresses, save either a manually entered or randomly generated Bearer token in the settings page; network listener settings cannot be saved until a token exists. A saved token is shown masked, can be copied directly into a caller configuration, and can be manually replaced or rotated. Every request to such addresses must provide that token. IPv6 addresses use brackets; link-local addresses also require a URL-encoded interface scope, for example `http://[fe80::1234%2512]:41833`.

The service uses HTTP and does not provide TLS. Do not expose it through the internet, port forwarding, public Wi-Fi, or an untrusted network. Even with a token, use it only on a private network you control.

The token is stored separately from the main configuration under the active config directory's `secrets` folder. It is not written to logs, diagnostics exports, or API responses, and it moves with the active configuration when portable mode changes.

## Authentication

The default loopback listener has no authentication requirement. `127.0.0.1` remains token-free when it is enabled alongside network listeners; only non-loopback addresses require this header for every endpoint:

```http
Authorization: Bearer <token from the settings page>
```

Missing or invalid tokens receive:

```json
{
  "error": {
    "code": "unauthorized",
    "message": "A valid Authorization bearer token is required."
  }
}
```

## Endpoints

Every response is `application/json; charset=utf-8`. The service sends no CORS headers. Browser integrations should use a same-machine extension, native bridge, or controlled backend rather than expose the API to arbitrary cross-origin web pages.

### `GET /v1/health`

Reports the API version and whether a snapshot is available.

```json
{
  "apiVersion": 1,
  "appVersion": "1.4.1",
  "snapshotAvailable": true
}
```

### `GET /v1/snapshot`

Returns the latest successful or usable `AppSnapshot`. Its fields use the desktop app's normalized snapshot shape, for example:

```json
{
  "schemaVersion": 1,
  "refreshedAt": "2026-08-19T12:00:00Z",
  "providers": [
    {
      "id": "codex-usage",
      "name": "Codex Usage",
      "status": "ok",
      "source": "remote",
      "windows": []
    }
  ]
}
```

If no snapshot exists yet, the endpoint returns `404` with `snapshot_unavailable`; temporary read failures return `503`. To avoid exposing Provider internals, every Provider's `metadata` and `diagnostics` are cleared to `null`, and error text is redacted before it is returned.

### `POST /v1/refresh`

Schedules a complete refresh asynchronously. An accepted request immediately returns `202`; poll `GET /v1/snapshot` for the result:

```json
{
  "accepted": true,
  "status": "scheduled"
}
```

If another API refresh is already running, it also returns `202` with:

```json
{
  "accepted": false,
  "status": "refresh-in-progress"
}
```

Refreshing uses the application's existing Provider execution, cache, retry, and concurrency protections. It cannot modify Provider configuration or credentials.

## Examples

For the default local listener:

```powershell
Invoke-RestMethod http://127.0.0.1:41833/v1/health
Invoke-RestMethod http://127.0.0.1:41833/v1/snapshot
Invoke-RestMethod -Method Post http://127.0.0.1:41833/v1/refresh
```

For a selected LAN interface, keep the token in the caller's own secure configuration and pass it in a request header:

```powershell
$headers = @{ Authorization = "Bearer <local integration API token>" }
Invoke-RestMethod -Headers $headers http://<selected-interface-address>:41833/v1/snapshot
```

Never commit a real token to code, script repositories, logs, or issue reports.
