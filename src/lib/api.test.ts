import { afterEach, expect, test } from "vitest";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import {
  getCachedSnapshot,
  getConfig,
  refreshProvider,
  refreshSnapshot,
  saveConfig,
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
