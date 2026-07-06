import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { App } from "./App";
import type { AppConfig, AppSnapshot } from "./types";

const mocks = vi.hoisted(() => {
  const config: AppConfig = {
    schemaVersion: 14,
    refreshIntervalSeconds: 300,
    displayMode: "remaining",
    lowQuotaWarningThreshold: 20,
    language: "en",
    remoteProviderRegistry: {
      registryUrl: null,
      providerProxyUrl: null,
      autoUpdate: true,
    },
    providers: [],
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
        windows: [],
      },
    ],
  };

  const configStorageInfo = {
    mode: "app-data",
    configPath:
      "C:\\Users\\tester\\AppData\\Roaming\\QuotaBarWin\\config.quotaBarWin.json",
    configDir: "C:\\Users\\tester\\AppData\\Roaming\\QuotaBarWin",
    appDataConfigPath:
      "C:\\Users\\tester\\AppData\\Roaming\\QuotaBarWin\\config.quotaBarWin.json",
    portableConfigPath: "C:\\Tools\\QuotaBarWin\\config.quotaBarWin.json",
    portableMarkerPath: "C:\\Tools\\QuotaBarWin\\quotabarwin.portable",
  };
  const listeners: {
    refreshRequested?: () => void;
    snapshotUpdated?: (snapshot: AppSnapshot) => void;
  } = {};

  return {
    listeners,
    getCachedSnapshot: vi.fn(async (): Promise<AppSnapshot | null> => null),
    getAppVersion: vi.fn(async () => "1.0.0(abc1234)"),
    getConfig: vi.fn(async () => config),
    getConfigStorageInfo: vi.fn(async () => configStorageInfo),
    getNetworkProxy: vi.fn(async () => null),
    getProviderPresets: vi.fn(async () => []),
    getTrayPopupPresentationId: vi.fn(async () => 0),
    listenForRefreshRequests: vi.fn(async (callback) => {
      listeners.refreshRequested = callback;
      return () => {
        if (listeners.refreshRequested === callback) {
          listeners.refreshRequested = undefined;
        }
      };
    }),
    listenForSnapshotUpdates: vi.fn(async (callback) => {
      listeners.snapshotUpdated = callback;
      return () => {
        if (listeners.snapshotUpdated === callback) {
          listeners.snapshotUpdated = undefined;
        }
      };
    }),
    listenForTrayPopupShown: vi.fn(async () => () => undefined),
    listenForSingleInstance: vi.fn(async () => () => undefined),
    hideCurrentWindow: vi.fn(async () => undefined),
    openConfigFolder: vi.fn(async () => undefined),
    openRemoteProviderGuide: vi.fn(async () => undefined),
    installRemoteProviderRegistry: vi.fn(async () => {
      throw new Error(
        "Install remote provider registry not available in browser preview",
      );
    }),
    installRemoteProviderManifest: vi.fn(async () => {
      throw new Error(
        "Install remote provider manifest not available in browser preview",
      );
    }),
    previewRemoteProviderRegistry: vi.fn(async () => []),
    removeRemoteProvider: vi.fn(async () => undefined),
    refreshRemoteProvider: vi.fn(async () => ({
      id: "",
      available: false,
      newChecksum: null,
    })),
    checkRemoteUpdates: vi.fn(async () => []),
    applyRemoteUpdate: vi.fn(async () => undefined),
    refreshProvider: vi.fn(async () => snapshot),
    refreshSnapshot: vi.fn(async () => snapshot),
    resetConfig: vi.fn(async () => config),
    saveConfig: vi.fn(async () => undefined),
    setNetworkProxy: vi.fn(async () => undefined),
    setPortableMode: vi.fn(async () => configStorageInfo),
  };
});

vi.mock("./lib/api", () => mocks);

beforeEach(() => {
  vi.clearAllMocks();
  mocks.listeners.refreshRequested = undefined;
  mocks.listeners.snapshotUpdated = undefined;
});

test("refresh_button_calls_refresh_snapshot", async () => {
  render(<App />);

  fireEvent.click(await screen.findByRole("button", { name: "Refresh" }));

  await waitFor(() => expect(mocks.refreshSnapshot).toHaveBeenCalledTimes(1));
});

test("main_app_refreshes_when_native_refresh_requested", async () => {
  render(<App />);

  await waitFor(() => expect(mocks.listeners.refreshRequested).toBeDefined());

  await act(async () => {
    mocks.listeners.refreshRequested?.();
  });

  await waitFor(() => expect(mocks.refreshSnapshot).toHaveBeenCalledTimes(1));
});

test("main_app_syncs_cached_snapshot_before_refreshing_when_shown", async () => {
  render(<App />);

  let resolveRefresh: ((snapshot: AppSnapshot) => void) | undefined;
  mocks.getCachedSnapshot.mockResolvedValueOnce({
    schemaVersion: 1,
    refreshedAt: "2026-06-08T10:05:00+08:00",
    providers: [
      {
        id: "cached-refresh",
        name: "Cached Refresh",
        status: "ok",
        source: "mock",
        updatedAt: "2026-06-08T10:05:00+08:00",
        error: null,
        diagnostics: null,
        metadata: null,
        windows: [],
      },
    ],
  });
  mocks.refreshSnapshot.mockImplementationOnce(
    () =>
      new Promise<AppSnapshot>((resolve) => {
        resolveRefresh = resolve;
      }),
  );

  await waitFor(() => expect(mocks.listeners.refreshRequested).toBeDefined());
  await act(async () => {
    mocks.listeners.refreshRequested?.();
  });

  await waitFor(() => expect(screen.getByText("Cached Refresh")).toBeInTheDocument());

  await act(async () => {
    resolveRefresh?.({
      schemaVersion: 1,
      refreshedAt: "2026-06-08T10:06:00+08:00",
      providers: [
        {
          id: "live-refresh",
          name: "Live Refresh",
          status: "ok",
          source: "mock",
          updatedAt: "2026-06-08T10:06:00+08:00",
          error: null,
          diagnostics: null,
          metadata: null,
          windows: [],
        },
      ],
    });
  });
});

test("main_app_shows_loading_instead_of_no_providers_before_first_snapshot", async () => {
  render(<App />);

  expect(await screen.findByTestId("global-status-strip")).toHaveTextContent(/Loading\.\.\.|加载中\.\.\./);
  expect(screen.queryByText("No providers configured. Add a provider in Settings.")).not.toBeInTheDocument();
  expect(mocks.refreshSnapshot).not.toHaveBeenCalled();
});

test("main_app_renders_cold_start_cache_before_native_refresh_arrives", async () => {
  mocks.getCachedSnapshot.mockResolvedValueOnce({
    schemaVersion: 1,
    refreshedAt: "2026-06-08T10:01:00+08:00",
    providers: [
      {
        id: "cold-cache",
        name: "Cold Cache",
        status: "ok",
        source: "mock",
        updatedAt: "2026-06-08T10:01:00+08:00",
        error: null,
        diagnostics: null,
        metadata: null,
        windows: [],
      },
    ],
  });

  render(<App />);

  expect(await screen.findByText("Cold Cache")).toBeInTheDocument();
  expect(mocks.refreshSnapshot).not.toHaveBeenCalled();

  await waitFor(() => expect(mocks.listeners.snapshotUpdated).toBeDefined());
  await act(async () => {
    mocks.listeners.snapshotUpdated?.({
      schemaVersion: 1,
      refreshedAt: "2026-06-08T10:02:00+08:00",
      providers: [
        {
          id: "live-refresh",
          name: "Live Refresh",
          status: "ok",
          source: "mock",
          updatedAt: "2026-06-08T10:02:00+08:00",
          error: null,
          diagnostics: null,
          metadata: null,
          windows: [],
        },
      ],
    });
  });

  expect(await screen.findByText("Live Refresh")).toBeInTheDocument();
});

test("main_app_uses_native_snapshot_updates_without_js_interval", async () => {
  const setIntervalSpy = vi.spyOn(window, "setInterval");
  const refreshCallsBeforeRender = mocks.refreshSnapshot.mock.calls.length;
  render(<App />);

  await waitFor(() => expect(mocks.listeners.snapshotUpdated).toBeDefined());
  expect(mocks.refreshSnapshot.mock.calls.length).toBe(refreshCallsBeforeRender);
  expect(
    setIntervalSpy.mock.calls.some(([, delay]) => delay === 300_000),
  ).toBe(false);

  await act(async () => {
    mocks.listeners.snapshotUpdated?.({
      schemaVersion: 1,
      refreshedAt: "2026-06-08T10:05:00+08:00",
      providers: [
        {
          id: "native-refresh",
          name: "Native Refresh",
          status: "ok",
          source: "mock",
          updatedAt: "2026-06-08T10:05:00+08:00",
          error: null,
          diagnostics: null,
          metadata: null,
          windows: [],
        },
      ],
    });
  });

  expect(screen.getByText("Native Refresh")).toBeInTheDocument();
  setIntervalSpy.mockRestore();
});

test("settings_replaces_provider_overview", async () => {
  render(<App />);

  await waitFor(() =>
    expect(
      screen.getByRole("heading", { name: "QuotaBarWin" }),
    ).toBeInTheDocument(),
  );
  await waitFor(() => expect(screen.getByText("v1.0.0")).toBeInTheDocument());
  expect(screen.getByTestId("global-status-strip")).toBeInTheDocument();
  expect(screen.getByLabelText("Providers")).toBeInTheDocument();
  expect(screen.getByText("v1.0.0")).toHaveAttribute("title", "Version 1.0.0(abc1234)");
  expect(screen.queryByText(/abc1234/)).not.toBeInTheDocument();

  expect(screen.getByRole("button", { name: "Overview" })).toHaveClass(
    "button-secondary",
  );
  expect(screen.getByRole("button", { name: "Settings" })).not.toHaveClass(
    "button-secondary",
  );

  fireEvent.click(screen.getByRole("button", { name: "Settings" }));

  expect(screen.getByLabelText("Settings")).toBeInTheDocument();
  expect(screen.getByRole("button", { name: "Refresh" })).toBeInTheDocument();
  expect(screen.getByRole("button", { name: "Overview" })).not.toHaveClass(
    "button-secondary",
  );
  expect(screen.getByRole("button", { name: "Settings" })).toHaveClass(
    "button-secondary",
  );
  expect(screen.queryByTestId("overview-page")).not.toBeInTheDocument();
});

test("saving_provider_reorder_projects_cached_snapshot_without_refreshing_data", async () => {
  const configWithTwoProviders: AppConfig = {
    schemaVersion: 14,
    refreshIntervalSeconds: 300,
    displayMode: "remaining",
    lowQuotaWarningThreshold: 20,
    language: "en",
    remoteProviderRegistry: {
      registryUrl: null,
      providerProxyUrl: null,
      autoUpdate: true,
    },
    providers: [
      {
        id: "remote-a",
        name: "Remote A",
        enabled: true,
        kind: "remote",
        manifestUrl: "https://example.test/a/provider.json",
        sourceUrl: "https://example.test/a/provider.cjs",
        runtime: "node",
        autoUpdate: false,
        updateIntervalSeconds: 3600,
        timeoutSeconds: 30,
      },
      {
        id: "remote-b",
        name: "Remote B",
        enabled: true,
        kind: "remote",
        manifestUrl: "https://example.test/b/provider.json",
        sourceUrl: "https://example.test/b/provider.cjs",
        runtime: "node",
        autoUpdate: false,
        updateIntervalSeconds: 3600,
        timeoutSeconds: 30,
      },
    ],
  };
  const cachedSnapshot: AppSnapshot = {
    schemaVersion: 1,
    refreshedAt: "2026-06-08T10:00:00+08:00",
    providers: [
      {
        id: "remote-a",
        name: "Remote A",
        status: "ok",
        source: "remote",
        updatedAt: "2026-06-08T10:00:00+08:00",
        error: null,
        diagnostics: null,
        metadata: null,
        windows: [],
      },
      {
        id: "remote-b",
        name: "Remote B",
        status: "ok",
        source: "remote",
        updatedAt: "2026-06-08T10:00:00+08:00",
        error: null,
        diagnostics: null,
        metadata: null,
        windows: [],
      },
    ],
  };
  mocks.getConfig.mockResolvedValueOnce(configWithTwoProviders);
  mocks.getCachedSnapshot.mockResolvedValueOnce(cachedSnapshot);

  render(<App />);

  expect(await screen.findByText("Remote A")).toBeInTheDocument();
  fireEvent.click(await screen.findByRole("button", { name: "Settings" }));
  fireEvent.click(screen.getAllByRole("button", { name: "More" })[1]);
  fireEvent.click(screen.getByRole("button", { name: "Up" }));
  fireEvent.click(screen.getByTestId("save-settings-button"));

  await waitFor(() => expect(mocks.saveConfig).toHaveBeenCalledTimes(1));
  await waitFor(() => expect(screen.getByTestId("overview-page")).toBeInTheDocument());
  expect(mocks.refreshSnapshot).not.toHaveBeenCalled();

  const overviewText = screen.getByLabelText("Providers").textContent ?? "";
  expect(overviewText.indexOf("Remote B")).toBeGreaterThanOrEqual(0);
  expect(overviewText.indexOf("Remote A")).toBeGreaterThanOrEqual(0);
  expect(overviewText.indexOf("Remote B")).toBeLessThan(overviewText.indexOf("Remote A"));
});
