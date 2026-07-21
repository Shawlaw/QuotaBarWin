import { afterEach, expect, test } from "vitest";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import {
  applyAppUpdate,
  checkRemoteUpdates,
  checkAppUpdate,
  downloadAppUpdate,
  getCachedSnapshot,
  getInstalledRemoteProviderManifest,
  installRemoteProviderManifest,
  installRemoteProviderRegistry,
  getConfig,
  getNetworkProxy,
  previewRemoteProviderRegistry,
  getTrayPopupPresentationId,
  refreshProvider,
  refreshRemoteProvider,
  refreshSnapshot,
  removeRemoteProvider,
  resetTrayPopupSize,
  saveConfig,
  setNetworkProxy,
  setTrayPopupAutoHeight,
  showMainWindow,
  startResizingCurrentWindow,
} from "./api";
import type { AppConfig, AppSnapshot } from "../types";

afterEach(() => {
  clearMocks();
});

const config: AppConfig = {
  schemaVersion: 14,
  refreshIntervalSeconds: 300,
  displayMode: "remaining",
  lowQuotaWarningThreshold: 20,
  language: "system",
  networkProxy: null,
  remoteProviderRegistry: {
    registryUrl: null,
    providerProxyUrl: null,
    autoUpdate: true,
  },
  providers: [],
};

const snapshot: AppSnapshot = {
  schemaVersion: 1,
  refreshedAt: "2026-06-09T10:00:00+08:00",
  providers: [],
};

test("api_invokes_core_snapshot_and_config_commands", async () => {
  const calls: Array<{ cmd: string; payload?: unknown }> = [];
  mockIPC((cmd, payload) => {
    calls.push({ cmd, payload });
    if (cmd === "refresh_snapshot") {
      return snapshot;
    }
    if (cmd === "get_cached_snapshot") {
      return null;
    }
    if (cmd === "get_config") {
      return config;
    }
    throw new Error(`unexpected command ${cmd}`);
  });

  await expect(refreshSnapshot()).resolves.toEqual(snapshot);
  await expect(getCachedSnapshot()).resolves.toBeNull();
  await expect(getConfig()).resolves.toEqual(config);
  expect(calls.map((call) => call.cmd)).toEqual([
    "refresh_snapshot",
    "get_cached_snapshot",
    "get_config",
  ]);
});

test("api_passes_refresh_provider_and_config_payloads_to_ipc", async () => {
  const payloads: Record<string, unknown> = {};
  mockIPC((cmd, payload) => {
    payloads[cmd] = payload;
    if (cmd === "refresh_provider") {
      return snapshot;
    }
    if (cmd === "save_config") {
      return null;
    }
    throw new Error(`unexpected command ${cmd}`);
  });

  await expect(refreshProvider("mock-codex")).resolves.toEqual(snapshot);
  await expect(saveConfig(config)).resolves.toBeNull();

  expect(payloads.refresh_provider).toEqual({ providerId: "mock-codex" });
  expect(payloads.save_config).toEqual({ config });
});

test("api_invokes_network_proxy_commands", async () => {
  const payloads: Record<string, unknown> = {};
  mockIPC((cmd, payload) => {
    payloads[cmd] = payload;
    if (cmd === "get_network_proxy") {
      return { kind: "http", url: "http://proxy.example.com:8080" };
    }
    if (cmd === "set_network_proxy") {
      return null;
    }
    throw new Error(`unexpected command ${cmd}`);
  });

  const proxy = await getNetworkProxy();
  await setNetworkProxy({
    kind: "socks5",
    url: "socks5://proxy.example.com:1080",
  });

  expect(proxy).toEqual({ kind: "http", url: "http://proxy.example.com:8080" });
  expect(payloads.set_network_proxy).toEqual({
    proxy: { kind: "socks5", url: "socks5://proxy.example.com:1080" },
  });
});

test("api_invokes_application_update_commands", async () => {
  const calls: Array<{ cmd: string; payload?: unknown }> = [];
  const update = {
    configured: true,
    currentVersion: "1.0.3",
    available: true,
    version: "1.0.4",
    notesUrl: "https://example.com/releases/v1.0.4",
    downloaded: false,
  };
  mockIPC((cmd, payload) => {
    calls.push({ cmd, payload });
    if (cmd === "check_app_update") {
      return update;
    }
    if (cmd === "download_app_update") {
      return { ...update, downloaded: true };
    }
    if (cmd === "apply_app_update") {
      return null;
    }
    throw new Error(`unexpected command ${cmd}`);
  });

  await expect(checkAppUpdate()).resolves.toEqual(update);
  await expect(downloadAppUpdate()).resolves.toEqual({ ...update, downloaded: true });
  await expect(applyAppUpdate()).resolves.toBeNull();
  expect(calls.map((call) => call.cmd)).toEqual([
    "check_app_update",
    "download_app_update",
    "apply_app_update",
  ]);
});

test("api_invokes_tray_popup_commands", async () => {
  const calls: Array<{ cmd: string; payload?: unknown }> = [];
  mockIPC((cmd, payload) => {
    calls.push({ cmd, payload });
    if (cmd === "get_tray_popup_presentation_id") {
      return 42;
    }
    if (cmd === "reset_tray_popup_size") {
      return null;
    }
    if (cmd === "show_main_window") {
      return null;
    }
    if (cmd === "start_tray_popup_resizing") {
      return null;
    }
    if (cmd === "set_tray_popup_auto_height") {
      return null;
    }
    throw new Error(`unexpected command ${cmd}`);
  });

  await expect(getTrayPopupPresentationId()).resolves.toBe(42);
  await expect(resetTrayPopupSize()).resolves.toBeNull();
  await expect(showMainWindow()).resolves.toBeNull();
  await expect(startResizingCurrentWindow()).resolves.toBeNull();
  await expect(setTrayPopupAutoHeight(420)).resolves.toBeNull();
  expect(calls.map((call) => call.cmd)).toEqual([
    "get_tray_popup_presentation_id",
    "reset_tray_popup_size",
    "show_main_window",
    "start_tray_popup_resizing",
    "set_tray_popup_auto_height",
  ]);
  expect(calls.at(-1)?.payload).toEqual({ height: 420 });
});

test("api_invokes_remote_provider_commands", async () => {
  const payloads: Record<string, unknown> = {};
  const registryResult = {
    installed: [
      {
        kind: "remote" as const,
        id: "remote-kimi",
        name: "Remote Kimi",
        enabled: true,
        manifestUrl: "https://example.com/provider.json",
        sourceUrl: "https://example.com/provider.cjs",
        runtime: "node",
        autoUpdate: true,
        updateIntervalSeconds: 3600,
        timeoutSeconds: 30,
      },
    ],
    skipped: [],
    failed: [],
  };
  const catalogResult = [
    {
      id: "remote-kimi",
      displayName: "Remote Kimi",
      version: "1.0.0",
      description: "Kimi usage",
      providerUrl: "https://example.com/provider.json",
      checksum: "sha256:manifest",
      installed: false,
      error: null,
    },
  ];
  const manifestResult = registryResult.installed[0];
  mockIPC((cmd, payload) => {
    payloads[cmd] = payload;
    if (cmd === "preview_remote_provider_registry") {
      return catalogResult;
    }
    if (cmd === "install_remote_provider_manifest") {
      return manifestResult;
    }
    if (cmd === "install_remote_provider_registry") {
      return registryResult;
    }
    if (cmd === "get_installed_remote_provider_manifest") {
      return {
        schemaVersion: 1,
        id: "remote-kimi",
        displayName: "Remote Kimi",
        runtime: "node",
        entry: "provider.cjs",
        requiredEnvVars: ["KIMI_API_KEY"],
        output: "provider-snapshot-v1",
        parameters: [{ name: "KIMI_API_KEY", kind: "secret", required: true }],
      };
    }
    if (cmd === "remove_remote_provider") {
      return null;
    }
    if (cmd === "refresh_remote_provider") {
      return { id: "remote-kimi", available: true, newChecksum: "sha256:abc" };
    }
    if (cmd === "check_remote_updates") {
      return [
        { id: "remote-kimi", available: true, newChecksum: "sha256:abc" },
      ];
    }
    throw new Error(`unexpected command ${cmd}`);
  });

  await expect(
    previewRemoteProviderRegistry(
      "https://example.com/registry.json",
      "http://proxy.example.com:8080",
    ),
  ).resolves.toEqual(catalogResult);
  await expect(
    installRemoteProviderManifest(
      "https://example.com/provider.json",
      "sha256:manifest",
      "http://proxy.example.com:8080",
      true,
    ),
  ).resolves.toEqual(manifestResult);
  await expect(
    installRemoteProviderRegistry(
      "https://example.com/registry.json",
      "http://proxy.example.com:8080",
      true,
    ),
  ).resolves.toEqual(registryResult);
  await expect(getInstalledRemoteProviderManifest("remote-kimi")).resolves.toMatchObject({
    id: "remote-kimi",
    parameters: [{ name: "KIMI_API_KEY", kind: "secret", required: true }],
  });
  await expect(removeRemoteProvider("remote-kimi")).resolves.toBeNull();
  await expect(refreshRemoteProvider("remote-kimi")).resolves.toEqual({
    id: "remote-kimi",
    available: true,
    newChecksum: "sha256:abc",
  });
  await expect(checkRemoteUpdates()).resolves.toEqual([
    { id: "remote-kimi", available: true, newChecksum: "sha256:abc" },
  ]);

  expect(payloads.install_remote_provider_registry).toEqual({
    url: "https://example.com/registry.json",
    proxyUrl: "http://proxy.example.com:8080",
    autoUpdate: true,
  });
  expect(payloads.preview_remote_provider_registry).toEqual({
    url: "https://example.com/registry.json",
    proxyUrl: "http://proxy.example.com:8080",
  });
  expect(payloads.install_remote_provider_manifest).toEqual({
    url: "https://example.com/provider.json",
    checksum: "sha256:manifest",
    proxyUrl: "http://proxy.example.com:8080",
    autoUpdate: true,
  });
  expect(payloads.get_installed_remote_provider_manifest).toEqual({ id: "remote-kimi" });
  expect(payloads.remove_remote_provider).toEqual({ id: "remote-kimi" });
  expect(payloads.refresh_remote_provider).toEqual({ id: "remote-kimi" });
});
