import { afterEach, expect, test } from "vitest";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import {
  checkRemoteUpdates,
  getCachedSnapshot,
  installRemoteProviderRegistry,
  getConfig,
  getNetworkProxy,
  getTrayPopupPresentationId,
  refreshProvider,
  refreshRemoteProvider,
  refreshSnapshot,
  removeRemoteProvider,
  resetTrayPopupSize,
  saveConfig,
  setNetworkProxy,
  startResizingCurrentWindow,
} from "./api";
import type { AppConfig, AppSnapshot } from "../types";

afterEach(() => {
  clearMocks();
});

const config: AppConfig = {
  schemaVersion: 13,
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

test("api_invokes_tray_popup_commands", async () => {
  const calls: string[] = [];
  mockIPC((cmd) => {
    calls.push(cmd);
    if (cmd === "get_tray_popup_presentation_id") {
      return 42;
    }
    if (cmd === "reset_tray_popup_size") {
      return null;
    }
    if (cmd === "start_tray_popup_resizing") {
      return null;
    }
    throw new Error(`unexpected command ${cmd}`);
  });

  await expect(getTrayPopupPresentationId()).resolves.toBe(42);
  await expect(resetTrayPopupSize()).resolves.toBeNull();
  await expect(startResizingCurrentWindow()).resolves.toBeNull();
  expect(calls).toEqual([
    "get_tray_popup_presentation_id",
    "reset_tray_popup_size",
    "start_tray_popup_resizing",
  ]);
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
      },
    ],
    skipped: [],
    failed: [],
  };
  mockIPC((cmd, payload) => {
    payloads[cmd] = payload;
    if (cmd === "install_remote_provider_registry") {
      return registryResult;
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
    installRemoteProviderRegistry(
      "https://example.com/registry.json",
      "http://proxy.example.com:8080",
      true,
    ),
  ).resolves.toEqual(registryResult);
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
  expect(payloads.remove_remote_provider).toEqual({ id: "remote-kimi" });
  expect(payloads.refresh_remote_provider).toEqual({ id: "remote-kimi" });
});
