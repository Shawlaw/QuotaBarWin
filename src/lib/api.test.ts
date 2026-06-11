import { afterEach, expect, test } from "vitest";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import {
  addRemoteProvider,
  checkRemoteUpdates,
  getCachedSnapshot,
  getConfig,
  getNetworkProxy,
  refreshProvider,
  refreshRemoteProvider,
  refreshSnapshot,
  removeRemoteProvider,
  saveConfig,
  setNetworkProxy,
  testProvider
} from "./api";
import type { AppConfig, AppSnapshot, ProviderConfig, ProviderSnapshot } from "../types";

afterEach(() => {
  clearMocks();
});

const config: AppConfig = {
  schemaVersion: 6,
  refreshIntervalSeconds: 300,
  displayMode: "remaining",
  lowQuotaWarningThreshold: 20,
  networkProxy: null,
  providers: [
    {
      kind: "mock",
      id: "mock-codex",
      name: "Codex Mock",
      enabled: true
    }
  ]
};

const snapshot: AppSnapshot = {
  schemaVersion: 1,
  refreshedAt: "2026-06-09T10:00:00+08:00",
  providers: []
};

const provider: ProviderSnapshot = {
  id: "mock-codex",
  name: "Codex Mock",
  status: "ok",
  source: "mock",
  updatedAt: null,
  windows: [],
  error: null,
  diagnostics: null,
  metadata: null
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
    "get_config"
  ]);
});

test("api_passes_provider_and_config_payloads_to_ipc", async () => {
  const payloads: Record<string, unknown> = {};
  const commandProvider: ProviderConfig = {
    kind: "command",
    id: "command",
    name: "Command",
    enabled: true,
    command: {
      executable: "node",
      args: ["fixtures/fake_provider_snapshot.js"],
      timeoutMs: 15000
    },
    parser: { type: "provider-snapshot" },
    windowLabelOverrides: {},
    visibleWindowIds: []
  };
  mockIPC((cmd, payload) => {
    payloads[cmd] = payload;
    if (cmd === "refresh_provider") {
      return snapshot;
    }
    if (cmd === "save_config") {
      return null;
    }
    if (cmd === "test_provider") {
      return provider;
    }
    throw new Error(`unexpected command ${cmd}`);
  });

  await expect(refreshProvider("command")).resolves.toEqual(snapshot);
  await expect(saveConfig(config)).resolves.toBeNull();
  await expect(testProvider(commandProvider)).resolves.toEqual(provider);

  expect(payloads.refresh_provider).toEqual({ providerId: "command" });
  expect(payloads.save_config).toEqual({ config });
  expect(payloads.test_provider).toEqual({ provider: commandProvider });
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
  await setNetworkProxy({ kind: "socks5", url: "socks5://proxy.example.com:1080" });

  expect(proxy).toEqual({ kind: "http", url: "http://proxy.example.com:8080" });
  expect(payloads.set_network_proxy).toEqual({
    proxy: { kind: "socks5", url: "socks5://proxy.example.com:1080" }
  });
});

test("api_invokes_remote_provider_commands", async () => {
  const payloads: Record<string, unknown> = {};
  const remoteProvider = {
    kind: "remote" as const,
    id: "remote-kimi",
    name: "Remote Kimi",
    enabled: true,
    manifestUrl: "https://example.com/provider.json",
    sourceUrl: "https://example.com/provider.cjs",
    runtime: "node",
    autoUpdate: true,
    updateIntervalSeconds: 3600
  };
  mockIPC((cmd, payload) => {
    payloads[cmd] = payload;
    if (cmd === "add_remote_provider") {
      return remoteProvider;
    }
    if (cmd === "remove_remote_provider") {
      return null;
    }
    if (cmd === "refresh_remote_provider") {
      return { id: "remote-kimi", available: true, newChecksum: "sha256:abc" };
    }
    if (cmd === "check_remote_updates") {
      return [{ id: "remote-kimi", available: true, newChecksum: "sha256:abc" }];
    }
    throw new Error(`unexpected command ${cmd}`);
  });

  await expect(
    addRemoteProvider("https://example.com/provider.json", "http://proxy.example.com:8080", true)
  ).resolves.toEqual(remoteProvider);
  await expect(removeRemoteProvider("remote-kimi")).resolves.toBeNull();
  await expect(refreshRemoteProvider("remote-kimi")).resolves.toEqual({
    id: "remote-kimi",
    available: true,
    newChecksum: "sha256:abc"
  });
  await expect(checkRemoteUpdates()).resolves.toEqual([
    { id: "remote-kimi", available: true, newChecksum: "sha256:abc" }
  ]);

  expect(payloads.add_remote_provider).toEqual({
    url: "https://example.com/provider.json",
    proxyUrl: "http://proxy.example.com:8080",
    autoUpdate: true
  });
  expect(payloads.remove_remote_provider).toEqual({ id: "remote-kimi" });
  expect(payloads.refresh_remote_provider).toEqual({ id: "remote-kimi" });
});
