import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { App, mergeProviderSetupSnapshot } from "./App";
import type { AppConfig, AppSnapshot, RemoteProviderConfig } from "./types";

const mocks = vi.hoisted(() => {
  const config: AppConfig = {
    schemaVersion: 17,
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
    appUpdateStatus?: (status: { info: unknown; animate: boolean }) => void;
    appUpdateNavigation?: (requestId: number) => void;
  } = {};

  return {
    listeners,
    dismissAppUpdateNotice: vi.fn(async () => ({
      configured: true, currentVersion: "1.0.0", available: false, version: null, notesUrl: null, downloaded: false,
    })),
    getCachedSnapshot: vi.fn(async (): Promise<AppSnapshot | null> => null),
    getApplicationUpdateNavigationRequest: vi.fn(async () => 0),
    getAppUpdateStatus: vi.fn(async () => ({
      configured: true, currentVersion: "1.0.0", available: false, version: null, notesUrl: null, downloaded: false,
    })),
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
    listenForAppUpdateStatus: vi.fn(async (callback) => {
      listeners.appUpdateStatus = callback;
      return () => {
        if (listeners.appUpdateStatus === callback) {
          listeners.appUpdateStatus = undefined;
        }
      };
    }),
    listenForApplicationUpdateRequests: vi.fn(async (callback) => {
      listeners.appUpdateNavigation = callback;
      return () => {
        if (listeners.appUpdateNavigation === callback) {
          listeners.appUpdateNavigation = undefined;
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
    openProjectGithub: vi.fn(async () => undefined),
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
    getInstalledRemoteProviderManifest: vi.fn(async () => ({
      schemaVersion: 1,
      id: "mock",
      displayName: "Mock",
      runtime: "node",
      entry: "provider.cjs",
      requiredEnvVars: [],
      output: "provider-snapshot-v1",
      parameters: [],
    })),
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
  mocks.listeners.appUpdateStatus = undefined;
  mocks.listeners.appUpdateNavigation = undefined;
});

test("refresh_button_calls_refresh_snapshot", async () => {
  render(<App />);

  expect(await screen.findByText("Codex Mock")).toBeInTheDocument();
  mocks.refreshSnapshot.mockClear();

  fireEvent.click(screen.getAllByRole("button", { name: "Refresh" })[0]);
  await waitFor(() => expect(mocks.refreshSnapshot).toHaveBeenCalledTimes(1));
});

test("main_app_opens_the_project_in_the_default_browser", async () => {
  render(<App />);

  expect(await screen.findByText("Codex Mock")).toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: "GitHub" }));

  expect(mocks.openProjectGithub).toHaveBeenCalledTimes(1);
});

test("main_app_animates_and_dismisses_an_automatic_update_notice", async () => {
  vi.spyOn(document, "hasFocus").mockReturnValue(true);
  render(<App />);

  await waitFor(() => expect(mocks.listeners.appUpdateStatus).toBeDefined());
  await act(async () => {
    mocks.listeners.appUpdateStatus?.({
      animate: true,
      info: {
        configured: true,
        currentVersion: "1.0.0",
        available: true,
        version: "1.1.0",
        notesUrl: "https://example.com/releases/v1.1.0",
        downloaded: false,
        dismissed: false,
      },
    });
  });

  const notice = await screen.findByTestId("app-update-notice");
  expect(notice).toHaveClass("app-update-notice--enter");
  expect(screen.queryByRole("button", { name: "Dismiss update notice" })).not.toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: "Later" }));
  await waitFor(() => expect(mocks.dismissAppUpdateNotice).toHaveBeenCalledTimes(1));
});

test("main_app_update_navigation opens and focuses the application update settings", async () => {
  const scrollIntoView = vi.fn();
  Object.defineProperty(HTMLElement.prototype, "scrollIntoView", {
    configurable: true,
    value: scrollIntoView,
  });
  render(<App />);

  await waitFor(() => expect(mocks.listeners.appUpdateNavigation).toBeDefined());
  await act(async () => {
    mocks.listeners.appUpdateNavigation?.(1);
  });

  expect(await screen.findByTestId("settings-page")).toBeInTheDocument();
  expect(screen.getByTestId("app-update-section")).toBeInTheDocument();
  await waitFor(() => expect(scrollIntoView).toHaveBeenCalledWith({ behavior: "smooth", block: "start" }));
  delete (HTMLElement.prototype as { scrollIntoView?: unknown }).scrollIntoView;
});

test("main_app_recovers_a_tray_update_navigation_request_when_the_main_window_regains_focus", async () => {
  const scrollIntoView = vi.fn();
  Object.defineProperty(HTMLElement.prototype, "scrollIntoView", {
    configurable: true,
    value: scrollIntoView,
  });
  render(<App />);

  await waitFor(() => expect(mocks.listeners.appUpdateNavigation).toBeDefined());
  mocks.getApplicationUpdateNavigationRequest.mockClear();
  mocks.getApplicationUpdateNavigationRequest.mockResolvedValueOnce(1);
  await act(async () => {
    window.dispatchEvent(new Event("focus"));
  });

  expect(await screen.findByTestId("settings-page")).toBeInTheDocument();
  await waitFor(() => expect(scrollIntoView).toHaveBeenCalledWith({ behavior: "smooth", block: "start" }));
  delete (HTMLElement.prototype as { scrollIntoView?: unknown }).scrollIntoView;
});

test("overview_and_settings_keep_independent_scroll_positions after application update navigation", async () => {
  const scrollIntoView = vi.fn();
  Object.defineProperty(HTMLElement.prototype, "scrollIntoView", {
    configurable: true,
    value: scrollIntoView,
  });
  render(<App />);

  await waitFor(() => expect(mocks.listeners.appUpdateNavigation).toBeDefined());
  const overviewScrollRegion = screen.getByTestId("overview-scroll-region");
  overviewScrollRegion.scrollTop = 286;
  fireEvent.scroll(overviewScrollRegion);
  await act(async () => {
    mocks.listeners.appUpdateNavigation?.(1);
  });
  expect(await screen.findByTestId("settings-page")).toBeInTheDocument();
  await waitFor(() => expect(scrollIntoView).toHaveBeenCalledTimes(1));
  const settingsScrollRegion = screen.getByTestId("settings-scroll-region");
  expect(settingsScrollRegion.scrollTop).toBe(0);
  settingsScrollRegion.scrollTop = 413;
  fireEvent.scroll(settingsScrollRegion);

  fireEvent.click(screen.getByRole("button", { name: "Overview" }));
  expect(await screen.findByTestId("overview-page")).toBeInTheDocument();
  expect(screen.getByTestId("overview-scroll-region").scrollTop).toBe(286);

  fireEvent.click(screen.getByRole("button", { name: "Settings" }));
  expect(await screen.findByTestId("settings-page")).toBeInTheDocument();
  expect(screen.getByTestId("settings-scroll-region").scrollTop).toBe(413);
  expect(scrollIntoView).toHaveBeenCalledTimes(1);

  delete (HTMLElement.prototype as { scrollIntoView?: unknown }).scrollIntoView;
});

test("main_app_refreshes_when_native_refresh_requested", async () => {
  render(<App />);

  expect(await screen.findByText("Codex Mock")).toBeInTheDocument();
  mocks.refreshSnapshot.mockClear();
  await waitFor(() => expect(mocks.listeners.refreshRequested).toBeDefined());

  await act(async () => {
    mocks.listeners.refreshRequested?.();
  });

  await waitFor(() => expect(mocks.refreshSnapshot).toHaveBeenCalledTimes(1));
});

test("main_app_syncs_cached_snapshot_before_refreshing_when_shown", async () => {
  render(<App />);

  expect(await screen.findByText("Codex Mock")).toBeInTheDocument();
  mocks.getCachedSnapshot.mockClear();
  mocks.refreshSnapshot.mockClear();

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

test("main_app_syncs_the_native_cache_when_the_window_regains_focus", async () => {
  render(<App />);

  expect(await screen.findByText("Codex Mock")).toBeInTheDocument();
  mocks.getCachedSnapshot.mockClear();
  mocks.refreshSnapshot.mockClear();
  mocks.getCachedSnapshot.mockResolvedValueOnce({
    schemaVersion: 1,
    refreshedAt: "2026-06-08T10:05:00+08:00",
    providers: [
      {
        id: "focused-cache",
        name: "Focused Cache",
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

  fireEvent.focus(window);

  expect(await screen.findByText("Focused Cache")).toBeInTheDocument();
  expect(mocks.getCachedSnapshot).toHaveBeenCalledTimes(1);
  expect(mocks.refreshSnapshot).not.toHaveBeenCalled();
});

test("main_app_syncs_the_native_cache_when_returning_to_the_overview", async () => {
  render(<App />);

  expect(await screen.findByText("Codex Mock")).toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: "Settings" }));
  expect(screen.getByTestId("settings-page")).toBeInTheDocument();

  mocks.getCachedSnapshot.mockClear();
  mocks.refreshSnapshot.mockClear();
  mocks.getCachedSnapshot.mockResolvedValueOnce({
    schemaVersion: 1,
    refreshedAt: "2026-06-08T10:05:00+08:00",
    providers: [
      {
        id: "overview-cache",
        name: "Overview Cache",
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

  fireEvent.click(screen.getByRole("button", { name: "Overview" }));

  expect(await screen.findByTestId("overview-page")).toBeInTheDocument();
  expect(await screen.findByText("Overview Cache")).toBeInTheDocument();
  expect(mocks.getCachedSnapshot).toHaveBeenCalledTimes(1);
  expect(mocks.refreshSnapshot).not.toHaveBeenCalled();
});

test("main_app_refreshes_after_config_load_when_no_first_snapshot", async () => {
  let resolveRefresh: ((snapshot: AppSnapshot) => void) | undefined;
  mocks.refreshSnapshot.mockImplementationOnce(
    () =>
      new Promise<AppSnapshot>((resolve) => {
        resolveRefresh = resolve;
      }),
  );

  render(<App />);

  expect(await screen.findByTestId("global-status-strip")).toHaveTextContent(/Loading\.\.\.|加载中\.\.\./);
  expect(screen.queryByText("No providers configured. Add a provider in Settings.")).not.toBeInTheDocument();
  await waitFor(() => expect(mocks.refreshSnapshot).toHaveBeenCalledTimes(1));

  await act(async () => {
    resolveRefresh?.({
      schemaVersion: 1,
      refreshedAt: "2026-06-08T10:04:00+08:00",
      providers: [
        {
          id: "startup-refresh",
          name: "Startup Refresh",
          status: "ok",
          source: "mock",
          updatedAt: "2026-06-08T10:04:00+08:00",
          error: null,
          diagnostics: null,
          metadata: null,
          windows: [],
        },
      ],
    });
  });

  expect(await screen.findByText("Startup Refresh")).toBeInTheDocument();
});

test("main_app_renders_cold_start_cache_before_native_refresh_arrives", async () => {
  const cachedSnapshot: AppSnapshot = {
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
  };
  mocks.getCachedSnapshot.mockResolvedValueOnce(cachedSnapshot);

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
  render(<App />);

  await waitFor(() => expect(mocks.listeners.snapshotUpdated).toBeDefined());
  expect(await screen.findByText("Codex Mock")).toBeInTheDocument();
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

  expect(await screen.findByText("Native Refresh")).toBeInTheDocument();
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

test("empty overview opens the provider catalog", async () => {
  render(<App />);

  expect(await screen.findByTestId("provider-setup-empty-state")).toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: "Add Provider" }));

  expect(await screen.findByTestId("add-provider-page")).toBeInTheDocument();
});

test("settings button returns from the provider catalog to the main settings page", async () => {
  render(<App />);

  fireEvent.click(await screen.findByRole("button", { name: "Add Provider" }));
  expect(await screen.findByTestId("add-provider-page")).toBeInTheDocument();

  fireEvent.click(screen.getByRole("button", { name: "Settings" }));

  await waitFor(() => expect(screen.queryByTestId("add-provider-page")).not.toBeInTheDocument());
  expect(screen.getByTestId("general-settings-section")).toBeInTheDocument();
});

test("provider setup test result fills an otherwise empty overview snapshot", () => {
  const config: AppConfig = {
    schemaVersion: 17,
    refreshIntervalSeconds: 300,
    displayMode: "remaining",
    lowQuotaWarningThreshold: 20,
    language: "en",
    remoteProviderRegistry: {
      registryUrl: null,
      providerProxyUrl: null,
      autoUpdate: true,
    },
    providers: [buildRemoteProvider("setup-provider", "Set up provider")],
  };

  const merged = mergeProviderSetupSnapshot(
    {
      schemaVersion: 1,
      refreshedAt: "2026-06-08T10:00:00+08:00",
      providers: [],
    },
    config,
    {
      id: "setup-provider",
      name: "Set up provider",
      status: "ok",
      source: "remote",
      updatedAt: "2026-06-08T10:01:00+08:00",
      error: null,
      diagnostics: null,
      metadata: null,
      windows: [],
    },
  );

  expect(merged?.providers).toHaveLength(1);
  expect(merged?.providers[0]).toMatchObject({
    id: "setup-provider",
    name: "Set up provider",
    status: "ok",
  });
});

test("leaving_settings_requires_resolving_unsaved_changes", async () => {
  render(<App />);

  await screen.findByRole("button", { name: "Settings" });
  fireEvent.click(screen.getByRole("button", { name: "Settings" }));
  fireEvent.change(screen.getByTestId("refresh-interval-input"), { target: { value: "120" } });
  fireEvent.click(screen.getByRole("button", { name: "Overview" }));

  expect(screen.getByRole("dialog", { name: "Unsaved changes" })).toBeInTheDocument();
  expect(screen.getByTestId("settings-page")).toBeInTheDocument();

  fireEvent.click(screen.getByTestId("cancel-unsaved-changes"));
  expect(screen.getByTestId("settings-page")).toBeInTheDocument();

  fireEvent.click(screen.getByRole("button", { name: "Overview" }));
  fireEvent.click(screen.getByTestId("discard-unsaved-changes"));
  await waitFor(() => expect(screen.getByTestId("overview-page")).toBeInTheDocument());
});

test("saving_provider_reorder_projects_cached_snapshot_without_refreshing_data", async () => {
  const configWithTwoProviders: AppConfig = {
    schemaVersion: 17,
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
  mocks.refreshSnapshot.mockResolvedValueOnce(cachedSnapshot);

  render(<App />);

  expect(await screen.findByText("Remote A")).toBeInTheDocument();
  fireEvent.click(await screen.findByRole("button", { name: "Settings" }));
  fireEvent.click(screen.getAllByRole("button", { name: "More" })[1]);
  fireEvent.click(screen.getByRole("button", { name: "Up" }));
  fireEvent.click(screen.getByTestId("save-settings-button"));

  await waitFor(() => expect(mocks.saveConfig).toHaveBeenCalledTimes(1));
  expect(screen.getByTestId("settings-page")).toBeInTheDocument();
  expect(screen.queryByTestId("overview-page")).not.toBeInTheDocument();
  expect(mocks.refreshSnapshot).not.toHaveBeenCalled();
  expect(mocks.refreshProvider).not.toHaveBeenCalled();

  fireEvent.click(screen.getByRole("button", { name: "Overview" }));
  await waitFor(() => expect(screen.getByTestId("overview-page")).toBeInTheDocument());
  expect(mocks.refreshSnapshot).not.toHaveBeenCalled();

  const overviewText = screen.getByLabelText("Providers").textContent ?? "";
  expect(overviewText.indexOf("Remote B")).toBeGreaterThanOrEqual(0);
  expect(overviewText.indexOf("Remote A")).toBeGreaterThanOrEqual(0);
  expect(overviewText.indexOf("Remote B")).toBeLessThan(overviewText.indexOf("Remote A"));
});

function buildRemoteProvider(id: string, name: string): RemoteProviderConfig {
  return {
    id,
    name,
    enabled: true,
    kind: "remote",
    manifestUrl: `https://example.test/${id}/provider.json`,
    sourceUrl: `https://example.test/${id}/provider.cjs`,
    runtime: "node",
    autoUpdate: false,
    updateIntervalSeconds: 3600,
    timeoutSeconds: 30,
  };
}

function buildTwoProviderConfig(): AppConfig {
  return {
    schemaVersion: 17,
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
      buildRemoteProvider("remote-a", "Remote A"),
      buildRemoteProvider("remote-b", "Remote B"),
    ],
  };
}

function buildTwoProviderSnapshot(): AppSnapshot {
  return {
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
}

test("saving_provider_env_var_change_refreshes_only_that_provider", async () => {
  mocks.getConfig.mockResolvedValueOnce(buildTwoProviderConfig());
  mocks.getCachedSnapshot.mockResolvedValueOnce(buildTwoProviderSnapshot());
  mocks.refreshSnapshot.mockResolvedValueOnce(buildTwoProviderSnapshot());

  render(<App />);

  expect(await screen.findByText("Remote A")).toBeInTheDocument();
  fireEvent.click(await screen.findByRole("button", { name: "Settings" }));
  fireEvent.click(screen.getByTestId("edit-provider-remote-a"));
  fireEvent.change(screen.getByLabelText("Environment variables"), {
    target: { value: "REMOTE_A_TOKEN=${secret:REMOTE_A_TOKEN}" },
  });
  fireEvent.click(screen.getByTestId("save-settings-button"));

  await waitFor(() => expect(mocks.saveConfig).toHaveBeenCalledTimes(1));
  await waitFor(() => expect(mocks.refreshProvider).toHaveBeenCalledWith("remote-a"));
  expect(mocks.refreshProvider).toHaveBeenCalledTimes(1);
  expect(mocks.refreshSnapshot).not.toHaveBeenCalled();
});

test("saving_network_proxy_change_triggers_full_refresh", async () => {
  mocks.getConfig.mockResolvedValueOnce(buildTwoProviderConfig());
  mocks.getCachedSnapshot.mockResolvedValueOnce(buildTwoProviderSnapshot());
  mocks.refreshSnapshot.mockResolvedValueOnce(buildTwoProviderSnapshot());

  render(<App />);

  expect(await screen.findByText("Remote A")).toBeInTheDocument();
  fireEvent.click(await screen.findByRole("button", { name: "Settings" }));
  fireEvent.change(screen.getByTestId("proxy-kind-select"), { target: { value: "http" } });
  fireEvent.change(screen.getByTestId("proxy-url-input"), {
    target: { value: "http://127.0.0.1:7890" },
  });
  fireEvent.click(screen.getByTestId("save-settings-button"));

  await waitFor(() => expect(mocks.saveConfig).toHaveBeenCalledTimes(1));
  await waitFor(() => expect(mocks.refreshSnapshot).toHaveBeenCalledTimes(1));
  expect(mocks.refreshProvider).not.toHaveBeenCalled();
});
