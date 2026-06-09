import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { App } from "./App";
import type { AppConfig, AppSnapshot } from "./types";

const mocks = vi.hoisted(() => {
  const config: AppConfig = {
    schemaVersion: 1,
    refreshIntervalSeconds: 300,
    displayMode: "remaining",
    lowQuotaWarningThreshold: 20,
    providers: [
      {
        id: "mock-codex",
        name: "Codex Mock",
        enabled: true,
        kind: "mock"
      }
    ]
  };

  const snapshot: AppSnapshot = {
    schemaVersion: 1,
    refreshedAt: "2026-06-08T10:00:00+08:00",
    providers: [
      {
        id: "mock-codex",
        name: "Codex Mock",
        status: "ok",
        source: "mock",
        updatedAt: "2026-06-08T10:00:00+08:00",
        error: null,
        diagnostics: null,
        metadata: null,
        windows: []
      }
    ]
  };

  const configStorageInfo = {
    mode: "app-data",
    configPath: "C:\\Users\\tester\\AppData\\Roaming\\QuotaBarWin\\config.quotaBarWin.json",
    configDir: "C:\\Users\\tester\\AppData\\Roaming\\QuotaBarWin",
    appDataConfigPath: "C:\\Users\\tester\\AppData\\Roaming\\QuotaBarWin\\config.quotaBarWin.json",
    portableConfigPath: "C:\\Tools\\QuotaBarWin\\config.quotaBarWin.json",
    portableMarkerPath: "C:\\Tools\\QuotaBarWin\\quotabarwin.portable"
  };

  return {
    getCachedSnapshot: vi.fn(async () => null),
    getAppVersion: vi.fn(async () => "0.0.0"),
    getConfig: vi.fn(async () => config),
    getConfigStorageInfo: vi.fn(async () => configStorageInfo),
    getProviderPresets: vi.fn(async () => []),
    listenForRefreshRequests: vi.fn(async () => () => undefined),
    listenForSingleInstance: vi.fn(async () => () => undefined),
    openConfigFolder: vi.fn(async () => undefined),
    refreshProvider: vi.fn(async () => snapshot),
    refreshSnapshot: vi.fn(async () => snapshot),
    resetConfig: vi.fn(async () => config),
    saveConfig: vi.fn(async () => undefined),
    setPortableMode: vi.fn(async () => configStorageInfo),
    testProvider: vi.fn(async () => snapshot.providers[0])
  };
});

vi.mock("./lib/api", () => mocks);

test("refresh_button_calls_refresh_snapshot", async () => {
  render(<App />);

  await waitFor(() => expect(mocks.refreshSnapshot).toHaveBeenCalledTimes(1));
  fireEvent.click(screen.getByRole("button", { name: "Refresh" }));

  await waitFor(() => expect(mocks.refreshSnapshot).toHaveBeenCalledTimes(2));
});

test("settings_replaces_provider_overview", async () => {
  render(<App />);

  await waitFor(() => expect(screen.getByRole("heading", { name: "QuotaBarWin V0.0.0" })).toBeInTheDocument());
  expect(screen.getByLabelText("Providers")).toBeInTheDocument();

  fireEvent.click(screen.getByRole("button", { name: "Settings" }));

  expect(screen.getByLabelText("Settings")).toBeInTheDocument();
  expect(screen.getByRole("button", { name: "Refresh" })).toBeInTheDocument();
  expect(screen.queryByLabelText("Providers")).not.toBeInTheDocument();
});
