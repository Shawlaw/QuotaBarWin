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

  await waitFor(() => expect(mocks.refreshSnapshot).toHaveBeenCalledTimes(1));
  fireEvent.click(screen.getAllByRole("button", { name: "Refresh" })[0]);

  await waitFor(() => expect(mocks.refreshSnapshot).toHaveBeenCalledTimes(2));
});

test("main_app_refreshes_when_native_refresh_requested", async () => {
  render(<App />);

  await waitFor(() => expect(mocks.refreshSnapshot).toHaveBeenCalledTimes(1));

  await act(async () => {
    mocks.listeners.refreshRequested?.();
  });

  await waitFor(() => expect(mocks.refreshSnapshot).toHaveBeenCalledTimes(2));
});

test("main_app_syncs_cached_snapshot_before_refreshing_when_shown", async () => {
  render(<App />);

  await waitFor(() => expect(mocks.refreshSnapshot).toHaveBeenCalledTimes(1));

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

test("main_app_uses_native_snapshot_updates_without_js_interval", async () => {
  const setIntervalSpy = vi.spyOn(window, "setInterval");
  const refreshCallsBeforeRender = mocks.refreshSnapshot.mock.calls.length;
  render(<App />);

  await waitFor(() =>
    expect(mocks.refreshSnapshot.mock.calls.length).toBeGreaterThan(
      refreshCallsBeforeRender,
    ),
  );
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
