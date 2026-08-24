import { useCallback, useEffect, useLayoutEffect, useRef, useState } from "react";
import { Header } from "./components/Header";
import { GlobalStatusStrip } from "./components/GlobalStatusStrip";
import { ProviderCard } from "./components/ProviderCard";
import { SettingsPanel } from "./components/SettingsPanel";
import { TrayPopup } from "./components/TrayPopup";
import { AppUpdateNotice } from "./components/AppUpdateNotice";
import {
  dismissAppUpdateNotice,
  getApplicationUpdateNavigationRequest,
  getCachedSnapshot,
  getAppUpdateStatus,
  getAppVersion,
  getConfig,
  getConfigStorageInfo,
  listenForRefreshRequests,
  listenForAppUpdateStatus,
  listenForApplicationUpdateRequests,
  listenForSnapshotUpdates,
  listenForSingleInstance,
  openConfigFolder,
  openProjectGithub,
  openRemoteProviderGuide,
  refreshProvider,
  refreshSnapshot,
  resetConfig,
  saveConfig,
  setPortableMode
} from "./lib/api";
import { I18nProvider, useI18n } from "./i18n";
import { applyAppTheme } from "./lib/theme";
import type {
  AppConfig,
  AppSnapshot,
  AppTheme,
  ConfigStorageInfo,
  ProviderSnapshot,
  ProviderSetupTestResult,
  RemoteProviderConfig
} from "./types";
import type { AppUpdateInfo, AppUpdateStatusEvent } from "./lib/api";

function fallbackSnapshot(error: unknown): AppSnapshot {
  return {
    schemaVersion: 1,
    refreshedAt: new Date().toISOString(),
    providers: [
      {
        id: "mock-error",
        name: "Codex Mock",
        status: "error",
        source: "mock",
        updatedAt: new Date().toISOString(),
        error: error instanceof Error ? error.message : "Unable to refresh snapshot",
        diagnostics: null,
        metadata: null,
        windows: []
      }
    ]
  };
}

function isTrayView(): boolean {
  return new URLSearchParams(window.location.search).get("view") === "tray";
}

function isSameAvailableAppUpdate(
  previous: AppUpdateInfo | null,
  next: AppUpdateInfo
): boolean {
  return (
    previous?.available === next.available &&
    previous?.version === next.version &&
    previous?.dismissed === next.dismissed
  );
}

function projectProviderSnapshot(
  provider: ProviderSnapshot,
  config: RemoteProviderConfig
): ProviderSnapshot {
  const visibleWindowIds = config.visibleWindowIds ?? [];
  const labelOverrides = config.windowLabelOverrides ?? {};
  const windows = provider.windows
    .filter((window) => visibleWindowIds.length === 0 || visibleWindowIds.includes(window.id))
    .map((window) => ({
      ...window,
      label: labelOverrides[window.id] ?? window.label
    }));

  return {
    ...provider,
    name: config.name,
    windows
  };
}

function projectSnapshotForConfig(
  snapshot: AppSnapshot | null,
  config: AppConfig
): AppSnapshot | null {
  if (!snapshot) {
    return snapshot;
  }

  const providersById = new Map(snapshot.providers.map((provider) => [provider.id, provider]));
  return {
    ...snapshot,
    providers: config.providers.flatMap((providerConfig) => {
      if (!providerConfig.enabled) {
        return [];
      }

      const provider = providersById.get(providerConfig.id);
      return provider ? [projectProviderSnapshot(provider, providerConfig)] : [];
    })
  };
}

export function mergeProviderSetupSnapshot(
  snapshot: AppSnapshot | null,
  config: AppConfig,
  testedProvider: ProviderSnapshot | null | undefined
): AppSnapshot | null {
  if (!testedProvider) {
    return projectSnapshotForConfig(snapshot, config);
  }

  const refreshedAt =
    testedProvider.updatedAt ?? snapshot?.refreshedAt ?? new Date().toISOString();
  const base: AppSnapshot = snapshot ?? {
    schemaVersion: 1,
    refreshedAt,
    providers: []
  };
  const withTestedProvider: AppSnapshot = {
    ...base,
    refreshedAt,
    providers: [
      ...base.providers.filter((provider) => provider.id !== testedProvider.id),
      testedProvider
    ]
  };
  return projectSnapshotForConfig(withTestedProvider, config);
}

function stableRecordEntries(record: Record<string, string> | undefined): [string, string][] {
  return Object.entries(record ?? {}).sort(([left], [right]) => left.localeCompare(right));
}

function providerRefreshSignature(provider: RemoteProviderConfig) {
  return {
    id: provider.id,
    enabled: provider.enabled,
    manifestUrl: provider.manifestUrl,
    sourceUrl: provider.sourceUrl,
    providerDir: provider.providerDir ?? null,
    runtime: provider.runtime,
    resolvedRuntime: provider.resolvedRuntime ?? null,
    proxyUrl: provider.proxyUrl ?? null,
    timeoutSeconds: provider.timeoutSeconds,
    trustedChecksum: provider.trustedChecksum ?? null,
    envVars: stableRecordEntries(provider.envVars)
  };
}

type DataRefreshTarget = "all" | string[];

function configRefreshTargets(
  previousConfig: AppConfig | null,
  nextConfig: AppConfig
): DataRefreshTarget | null {
  if (!previousConfig) {
    return "all";
  }

  if (JSON.stringify(previousConfig.networkProxy ?? null) !== JSON.stringify(nextConfig.networkProxy ?? null)) {
    return "all";
  }

  const previousProviders = new Map(previousConfig.providers.map((provider) => [provider.id, provider]));
  const providerIds: string[] = [];
  for (const nextProvider of nextConfig.providers) {
    const previousProvider = previousProviders.get(nextProvider.id);
    if (!previousProvider) {
      if (nextProvider.enabled) {
        providerIds.push(nextProvider.id);
      }
      continue;
    }

    if (!previousProvider.enabled && nextProvider.enabled) {
      providerIds.push(nextProvider.id);
      continue;
    }

    if (!nextProvider.enabled) {
      continue;
    }

    if (
      JSON.stringify(providerRefreshSignature(previousProvider)) !==
      JSON.stringify(providerRefreshSignature(nextProvider))
    ) {
      providerIds.push(nextProvider.id);
    }
  }

  return providerIds.length > 0 ? providerIds : null;
}

export function App() {
  const [frontendLanguage, setFrontendLanguage] = useState<AppConfig["language"]>("zh-CN");
  const [theme, setTheme] = useState<AppTheme>("system");
  const trayView = isTrayView();

  useEffect(() => {
    const mediaQuery = window.matchMedia?.("(prefers-color-scheme: dark)");
    const syncTheme = () => applyAppTheme(theme);
    syncTheme();

    if (theme !== "system" || !mediaQuery) {
      return;
    }

    mediaQuery.addEventListener("change", syncTheme);
    return () => mediaQuery.removeEventListener("change", syncTheme);
  }, [theme]);

  useEffect(() => {
    if (!trayView) {
      return;
    }

    let isMounted = true;
    void getConfig().then((config) => {
      if (isMounted) {
        setFrontendLanguage(config.language);
        setTheme(config.theme ?? "system");
      }
    });

    return () => {
      isMounted = false;
    };
  }, [trayView]);

  return (
    <I18nProvider language={frontendLanguage}>
      {trayView ? (
        <TrayPopup onThemeChange={setTheme} />
      ) : (
        <MainApp onLanguageChange={setFrontendLanguage} onThemeChange={setTheme} />
      )}
    </I18nProvider>
  );
}

type MainAppProps = {
  onLanguageChange: (language: AppConfig["language"]) => void;
  onThemeChange: (theme: AppTheme) => void;
};

function MainApp({ onLanguageChange, onThemeChange }: MainAppProps) {
  const { t } = useI18n();
  const refreshInFlight = useRef(false);
  const queuedGlobalRefresh = useRef(false);
  const persistedConfigRef = useRef<AppConfig | null>(null);
  const [snapshot, setSnapshot] = useState<AppSnapshot | null>(null);
  const [config, setConfig] = useState<AppConfig | null>(null);
  const [configStorageInfo, setConfigStorageInfo] = useState<ConfigStorageInfo | null>(null);
  const [appVersion, setAppVersion] = useState<string>("unknown");
  const [isLoading, setIsLoading] = useState(false);
  const [refreshingProviderIds, setRefreshingProviderIds] = useState<Record<string, boolean>>({});
  const [isSaving, setIsSaving] = useState(false);
  const [isConfigStorageBusy, setIsConfigStorageBusy] = useState(false);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [initialProviderSettingsView, setInitialProviderSettingsView] = useState<"main" | "add">("main");
  const [settingsCloseRequest, setSettingsCloseRequest] = useState(0);
  const [settingsHomeRequest, setSettingsHomeRequest] = useState(0);
  const [appUpdateInfo, setAppUpdateInfo] = useState<AppUpdateInfo | null>(null);
  const [appUpdateNoticeSequence, setAppUpdateNoticeSequence] = useState(0);
  const [appUpdateFocusRequest, setAppUpdateFocusRequest] = useState(0);
  const appUpdateStatusRevisionRef = useRef(0);
  const appUpdateInfoRef = useRef<AppUpdateInfo | null>(null);
  const handledAppUpdateNavigationRequestRef = useRef(0);
  const settingsOpenRef = useRef(false);
  const overviewScrollRegionRef = useRef<HTMLDivElement>(null);
  const settingsScrollRegionRef = useRef<HTMLDivElement>(null);
  const overviewScrollTopRef = useRef(0);
  const settingsScrollTopRef = useRef(0);
  const isShowingSettings = settingsOpen && config !== null;

  useLayoutEffect(() => {
    const scrollRegion = isShowingSettings
      ? settingsScrollRegionRef.current
      : overviewScrollRegionRef.current;
    const scrollTop = isShowingSettings
      ? settingsScrollTopRef.current
      : overviewScrollTopRef.current;
    if (scrollRegion) {
      scrollRegion.scrollTop = scrollTop;
    }
  }, [isShowingSettings]);

  const syncCachedSnapshot = useCallback(async () => {
    try {
      const cached = await getCachedSnapshot();
      if (cached) {
        setSnapshot(cached);
      }
      return cached;
    } catch {
      return null;
    }
  }, []);

  const syncAppUpdateStatus = useCallback(async () => {
    const requestRevision = appUpdateStatusRevisionRef.current + 1;
    appUpdateStatusRevisionRef.current = requestRevision;
    try {
      const info = await getAppUpdateStatus();
      if (requestRevision === appUpdateStatusRevisionRef.current) {
        appUpdateInfoRef.current = info;
        setAppUpdateInfo(info);
      }
    } catch {
      // A transient status read must not replace the last known update notice.
    }
  }, []);

  const applyAppUpdateStatus = useCallback((status: AppUpdateStatusEvent) => {
    const isDuplicateAvailableUpdate = isSameAvailableAppUpdate(
      appUpdateInfoRef.current,
      status.info
    );
    appUpdateStatusRevisionRef.current += 1;
    appUpdateInfoRef.current = status.info;
    setAppUpdateInfo(status.info);
    if (
      status.animate &&
      status.info.available &&
      !status.info.dismissed &&
      !isDuplicateAvailableUpdate &&
      document.hasFocus()
    ) {
      setAppUpdateNoticeSequence((current) => current + 1);
    }
  }, []);

  const loadSnapshot = useCallback(async () => {
    if (refreshInFlight.current) {
      // Coalesce refresh requests arriving while a refresh is running into a
      // single trailing run instead of dropping them silently.
      queuedGlobalRefresh.current = true;
      return;
    }

    do {
      queuedGlobalRefresh.current = false;
      refreshInFlight.current = true;
      setIsLoading(true);
      try {
        await syncCachedSnapshot();
        setSnapshot(await refreshSnapshot());
      } catch (error) {
        const cached = await syncCachedSnapshot();
        setSnapshot(cached ?? fallbackSnapshot(error));
      } finally {
        refreshInFlight.current = false;
        setIsLoading(false);
      }
    } while (queuedGlobalRefresh.current);
  }, [syncCachedSnapshot]);

  useEffect(() => {
    let isMounted = true;

    void getCachedSnapshot()
      .then((cached) => {
        if (cached && isMounted) {
          setSnapshot(cached);
        }
      })
      .catch(() => undefined);

    async function initialize() {
      try {
        const [loadedConfig, loadedVersion, loadedStorageInfo] = await Promise.all([
          getConfig(),
          getAppVersion(),
          getConfigStorageInfo()
        ]);
        if (!isMounted) {
          return;
        }
        onLanguageChange(loadedConfig.language);
        persistedConfigRef.current = loadedConfig;
        setConfig(loadedConfig);
        setConfigStorageInfo(loadedStorageInfo);
        setAppVersion(loadedVersion);
      } catch {
        // Cache loading and native refresh events still keep the overview usable.
      }
    }

    void initialize();

    return () => {
      isMounted = false;
    };
  }, [onLanguageChange]);

  useEffect(() => {
    if (config) {
      onLanguageChange(config.language);
      onThemeChange(config.theme ?? "system");
    }
  }, [config, onLanguageChange, onThemeChange]);

  useEffect(() => {
    if (!config || snapshot !== null || refreshInFlight.current) {
      return;
    }

    void loadSnapshot();
  }, [config, loadSnapshot, snapshot]);

  useEffect(() => {
    // The background scheduler owns network refreshes even while the main
    // window is hidden. When this window becomes active again, read the
    // native cache so the overview immediately reflects its latest result.
    const syncOnFocus = () => {
      void syncCachedSnapshot();
      void syncAppUpdateStatus();
    };
    window.addEventListener("focus", syncOnFocus);
    return () => window.removeEventListener("focus", syncOnFocus);
  }, [syncAppUpdateStatus, syncCachedSnapshot]);

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    void listenForRefreshRequests(() => void loadSnapshot()).then((cleanup) => {
      unlisten = cleanup;
    });

    return () => {
      unlisten?.();
    };
  }, [loadSnapshot]);

  useEffect(() => {
    let isMounted = true;
    let unlisten: (() => void) | undefined;
    const handleRequest = (requestId: number) => {
      if (!isMounted || requestId <= handledAppUpdateNavigationRequestRef.current) {
        return;
      }
      handledAppUpdateNavigationRequestRef.current = requestId;
      openApplicationUpdate();
    };
    void getApplicationUpdateNavigationRequest().then(handleRequest).catch(() => undefined);
    void listenForApplicationUpdateRequests(handleRequest).then((cleanup) => {
      unlisten = cleanup;
    });
    // The tray command records its request before it focuses this window. Reading that durable
    // request on focus makes the tray route work even if Tauri delivers the transient event
    // while this renderer is resuming from a hidden state.
    const recoverRequestOnFocus = () => {
      void getApplicationUpdateNavigationRequest().then(handleRequest).catch(() => undefined);
    };
    window.addEventListener("focus", recoverRequestOnFocus);

    return () => {
      isMounted = false;
      unlisten?.();
      window.removeEventListener("focus", recoverRequestOnFocus);
    };
  }, []);

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    void listenForSnapshotUpdates((updatedSnapshot) => setSnapshot(updatedSnapshot)).then((cleanup) => {
      unlisten = cleanup;
    });

    return () => {
      unlisten?.();
    };
  }, []);

  useEffect(() => {
    let isMounted = true;
    let unlisten: (() => void) | undefined;
    void syncAppUpdateStatus();
    void listenForAppUpdateStatus((status) => {
      if (!isMounted) {
        return;
      }
      applyAppUpdateStatus(status);
    }).then((cleanup) => {
      unlisten = cleanup;
    });

    return () => {
      isMounted = false;
      unlisten?.();
    };
  }, [applyAppUpdateStatus, syncAppUpdateStatus]);

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    void listenForSingleInstance((message) => window.alert(message)).then((cleanup) => {
      unlisten = cleanup;
    });

    return () => {
      unlisten?.();
    };
  }, []);

  const dataRefreshActive = useRef(false);
  const dataRefreshPending = useRef<"all" | Set<string> | null>(null);

  async function runDataRefresh(target: "all" | Set<string>) {
    dataRefreshActive.current = true;
    try {
      if (target === "all") {
        await loadSnapshot();
      } else {
        for (const providerId of target) {
          setRefreshingProviderIds((current) => ({ ...current, [providerId]: true }));
          try {
            setSnapshot(await refreshProvider(providerId));
          } catch (error) {
            const cached = await getCachedSnapshot();
            setSnapshot(cached ?? fallbackSnapshot(error));
          } finally {
            setRefreshingProviderIds((current) => {
              const next = { ...current };
              delete next[providerId];
              return next;
            });
          }
        }
      }
    } finally {
      dataRefreshActive.current = false;
      const pending = dataRefreshPending.current;
      dataRefreshPending.current = null;
      if (pending) {
        void runDataRefresh(pending);
      }
    }
  }

  // Fire-and-forget refresh after a config save. Requests arriving while a
  // save-triggered refresh is running are merged into at most one queued run.
  function queueDataRefresh(target: DataRefreshTarget) {
    if (dataRefreshActive.current) {
      const pending = dataRefreshPending.current;
      if (pending === "all" || target === "all") {
        dataRefreshPending.current = "all";
      } else {
        dataRefreshPending.current = new Set([...(pending ?? []), ...target]);
      }
      return;
    }

    void runDataRefresh(target === "all" ? "all" : new Set(target));
  }

  function synchronizePersistedConfig(
    updatedConfig: AppConfig,
    testResult?: ProviderSetupTestResult
  ) {
    const refreshTarget = configRefreshTargets(persistedConfigRef.current, updatedConfig);
    persistedConfigRef.current = updatedConfig;
    setConfig(updatedConfig);
    setSnapshot((current) =>
      mergeProviderSetupSnapshot(current, updatedConfig, testResult?.provider)
    );
    if (refreshTarget) {
      queueDataRefresh(refreshTarget);
    }
  }

  async function persistConfig() {
    if (!config) {
      return;
    }

    setIsSaving(true);
    try {
      const refreshTarget = configRefreshTargets(persistedConfigRef.current, config);
      await saveConfig(config);
      persistedConfigRef.current = config;
      setConfigStorageInfo(await getConfigStorageInfo());
      setSnapshot((current) => projectSnapshotForConfig(current, config));
      if (refreshTarget) {
        queueDataRefresh(refreshTarget);
      }
    } finally {
      setIsSaving(false);
    }
  }

  async function togglePortableMode(enabled: boolean) {
    if (!config) {
      return;
    }

    setIsConfigStorageBusy(true);
    try {
      await saveConfig(config);
      const storageInfo = await setPortableMode(enabled);
      setConfigStorageInfo(storageInfo);
      const updatedConfig = await getConfig();
      persistedConfigRef.current = updatedConfig;
      setConfig(updatedConfig);
      await loadSnapshot();
    } finally {
      setIsConfigStorageBusy(false);
    }
  }

  async function restoreDefaultConfig() {
    setIsConfigStorageBusy(true);
    try {
      const defaultConfig = await resetConfig();
      persistedConfigRef.current = defaultConfig;
      setConfig(defaultConfig);
      setConfigStorageInfo(await getConfigStorageInfo());
      await loadSnapshot();
    } finally {
      setIsConfigStorageBusy(false);
    }
  }

  async function refreshSingleProvider(providerId: string) {
    if (refreshingProviderIds[providerId]) {
      return;
    }

    setRefreshingProviderIds((current) => ({ ...current, [providerId]: true }));
    try {
      setSnapshot(await refreshProvider(providerId));
    } catch (error) {
      const cached = await getCachedSnapshot();
      setSnapshot(cached ?? fallbackSnapshot(error));
    } finally {
      setRefreshingProviderIds((current) => {
        const next = { ...current };
        delete next[providerId];
        return next;
      });
    }
  }

  const hasSnapshot = snapshot !== null;
  const providers = snapshot?.providers ?? [];

  function rememberActiveScrollPosition() {
    if (settingsOpenRef.current) {
      if (settingsScrollRegionRef.current) {
        settingsScrollTopRef.current = settingsScrollRegionRef.current.scrollTop;
      }
    } else if (overviewScrollRegionRef.current) {
      overviewScrollTopRef.current = overviewScrollRegionRef.current.scrollTop;
    }
  }

  function openSettings(initialView: "main" | "add") {
    rememberActiveScrollPosition();
    settingsOpenRef.current = true;
    setInitialProviderSettingsView(initialView);
    setSettingsOpen(true);
  }

  function closeSettings() {
    rememberActiveScrollPosition();
    settingsOpenRef.current = false;
    setSettingsOpen(false);
  }

  function openApplicationUpdate() {
    openSettings("main");
    setSettingsHomeRequest((current) => current + 1);
    setAppUpdateFocusRequest((current) => current + 1);
  }

  function dismissApplicationUpdate() {
    void dismissAppUpdateNotice()
      .then((info) => applyAppUpdateStatus({ info, animate: false }))
      .catch(() => undefined);
  }

  return (
    <main className="app-shell">
      <Header
        activeView={settingsOpen ? "settings" : "overview"}
        appVersion={appVersion}
        isLoading={isLoading}
        onOpenOverview={() => {
          // Returning to the overview should be instant and must not depend on
          // a new network refresh. The native command returns its in-memory
          // snapshot cache before falling back to the on-disk cache.
          void syncCachedSnapshot();
          if (settingsOpen) {
            setSettingsCloseRequest((current) => current + 1);
          }
        }}
        onOpenSettings={() => {
          if (settingsOpen) {
            setSettingsHomeRequest((current) => current + 1);
          } else {
            openSettings("main");
          }
        }}
        onRefresh={loadSnapshot}
        onOpenGithub={() => void openProjectGithub()}
      />
      <AppUpdateNotice
        key={appUpdateNoticeSequence}
        animate={appUpdateNoticeSequence > 0}
        info={appUpdateInfo}
        onDismiss={dismissApplicationUpdate}
        onOpenUpdate={openApplicationUpdate}
      />
      {isShowingSettings ? (
        <div
          className="app-view-scroll-region"
          data-testid="settings-scroll-region"
          ref={settingsScrollRegionRef}
          onScroll={(event) => {
            settingsScrollTopRef.current = event.currentTarget.scrollTop;
          }}
        >
          <SettingsPanel
            appVersion={appVersion}
            config={config}
            configStorageInfo={configStorageInfo}
            isConfigStorageBusy={isConfigStorageBusy}
            isSaving={isSaving}
            snapshotProviders={providers}
            onChange={setConfig}
            onOpenConfigFolder={openConfigFolder}
            onResetConfig={restoreDefaultConfig}
            onSave={persistConfig}
            onSetPortableMode={(enabled) => void togglePortableMode(enabled)}
            onPersistedConfigChanged={synchronizePersistedConfig}
            onRequestClose={closeSettings}
            closeRequest={settingsCloseRequest}
            settingsHomeRequest={settingsHomeRequest}
            appUpdateFocusRequest={appUpdateFocusRequest}
            onAppUpdateFocusHandled={() => setAppUpdateFocusRequest(0)}
            onAppUpdateStatusChange={(info) =>
              applyAppUpdateStatus({ info, animate: true })
            }
            initialProviderSettingsView={initialProviderSettingsView}
          />
        </div>
      ) : (
        <div
          className="app-view-scroll-region"
          data-testid="overview-scroll-region"
          ref={overviewScrollRegionRef}
          onScroll={(event) => {
            overviewScrollTopRef.current = event.currentTarget.scrollTop;
          }}
        >
          <section className="overview-page" aria-label={t.app.overviewLabel} data-testid="overview-page">
            {hasSnapshot ? (
              <GlobalStatusStrip
                providers={providers}
                refreshedAt={snapshot.refreshedAt}
                refreshIntervalSeconds={config?.refreshIntervalSeconds}
                lowQuotaWarningThreshold={config?.lowQuotaWarningThreshold}
              />
            ) : (
              <section className="global-status" data-testid="global-status-strip">
                {t.settings.loading}
              </section>
            )}
            <section className="provider-list" aria-label={t.app.providersLabel}>
              {config && !config.providers.some((provider) =>
                provider.enabled && provider.setupState !== "pending" && provider.setupState !== "unverified"
              ) ? (
                <section className="settings-empty provider-setup-empty" data-testid="provider-setup-empty-state">
                  <h2>{t.providerSetup.noUsableProvidersTitle}</h2>
                  <p>{t.providerSetup.noUsableProvidersBody}</p>
                  <div className="settings-actions">
                    <button
                      type="button"
                      className="button-primary"
                      onClick={() => openSettings("add")}
                    >
                      {t.providerSetup.addProvider}
                    </button>
                    <button
                      type="button"
                      className="button-secondary"
                      onClick={() => void openRemoteProviderGuide()}
                    >
                      {t.providerSetup.openGuide}
                    </button>
                  </div>
                </section>
              ) : null}
              {providers.map((provider) => (
                <ProviderCard
                  key={provider.id}
                  provider={provider}
                  displayMode={config?.displayMode ?? "remaining"}
                  lowQuotaWarningThreshold={config?.lowQuotaWarningThreshold}
                  isRefreshing={refreshingProviderIds[provider.id] ?? false}
                  onRefresh={() => void refreshSingleProvider(provider.id)}
                />
              ))}
            </section>
          </section>
        </div>
      )}
    </main>
  );
}
