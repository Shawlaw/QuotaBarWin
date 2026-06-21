import { useCallback, useEffect, useRef, useState } from "react";
import { Header } from "./components/Header";
import { GlobalStatusStrip } from "./components/GlobalStatusStrip";
import { ProviderCard } from "./components/ProviderCard";
import { SettingsPanel } from "./components/SettingsPanel";
import { TrayPopup } from "./components/TrayPopup";
import {
  getCachedSnapshot,
  getAppVersion,
  getConfig,
  getConfigStorageInfo,
  listenForRefreshRequests,
  listenForSnapshotUpdates,
  listenForSingleInstance,
  openConfigFolder,
  refreshProvider,
  refreshSnapshot,
  resetConfig,
  saveConfig,
  setPortableMode
} from "./lib/api";
import { I18nProvider, useI18n } from "./i18n";
import type { AppConfig, AppSnapshot, ConfigStorageInfo } from "./types";

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

export function App() {
  const [frontendLanguage, setFrontendLanguage] = useState<AppConfig["language"]>("zh-CN");
  const trayView = isTrayView();

  useEffect(() => {
    if (!trayView) {
      return;
    }

    let isMounted = true;
    void getConfig().then((config) => {
      if (isMounted) {
        setFrontendLanguage(config.language);
      }
    });

    return () => {
      isMounted = false;
    };
  }, [trayView]);

  return (
    <I18nProvider language={frontendLanguage}>
      {trayView ? <TrayPopup /> : <MainApp onLanguageChange={setFrontendLanguage} />}
    </I18nProvider>
  );
}

type MainAppProps = {
  onLanguageChange: (language: AppConfig["language"]) => void;
};

function MainApp({ onLanguageChange }: MainAppProps) {
  const { t } = useI18n();
  const refreshInFlight = useRef(false);
  const [snapshot, setSnapshot] = useState<AppSnapshot | null>(null);
  const [config, setConfig] = useState<AppConfig | null>(null);
  const [configStorageInfo, setConfigStorageInfo] = useState<ConfigStorageInfo | null>(null);
  const [appVersion, setAppVersion] = useState<string>("unknown");
  const [isLoading, setIsLoading] = useState(false);
  const [refreshingProviderIds, setRefreshingProviderIds] = useState<Record<string, boolean>>({});
  const [isSaving, setIsSaving] = useState(false);
  const [isConfigStorageBusy, setIsConfigStorageBusy] = useState(false);
  const [settingsOpen, setSettingsOpen] = useState(false);

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

  const loadSnapshot = useCallback(async () => {
    if (refreshInFlight.current) {
      return;
    }

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
  }, [syncCachedSnapshot]);

  useEffect(() => {
    let isMounted = true;

    async function initialize() {
      try {
        const loadedConfig = await getConfig();
        const loadedVersion = await getAppVersion();
        const loadedStorageInfo = await getConfigStorageInfo();
        if (!isMounted) {
          return;
        }
        onLanguageChange(loadedConfig.language);
        setConfig(loadedConfig);
        setConfigStorageInfo(loadedStorageInfo);
        setAppVersion(loadedVersion);
        const cached = await getCachedSnapshot();
        if (cached && isMounted) {
          setSnapshot(cached);
        }
      } finally {
        if (isMounted) {
          void loadSnapshot();
        }
      }
    }

    void initialize();

    return () => {
      isMounted = false;
    };
  }, [loadSnapshot]);

  useEffect(() => {
    if (config) {
      onLanguageChange(config.language);
    }
  }, [config, onLanguageChange]);

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
    let unlisten: (() => void) | undefined;
    void listenForSnapshotUpdates((updatedSnapshot) => setSnapshot(updatedSnapshot)).then((cleanup) => {
      unlisten = cleanup;
    });

    return () => {
      unlisten?.();
    };
  }, []);

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    void listenForSingleInstance((message) => window.alert(message)).then((cleanup) => {
      unlisten = cleanup;
    });

    return () => {
      unlisten?.();
    };
  }, []);

  async function persistConfig() {
    if (!config) {
      return;
    }

    setIsSaving(true);
    try {
      await saveConfig(config);
      setConfigStorageInfo(await getConfigStorageInfo());
      setSettingsOpen(false);
      await loadSnapshot();
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
      setConfig(await getConfig());
      await loadSnapshot();
    } finally {
      setIsConfigStorageBusy(false);
    }
  }

  async function restoreDefaultConfig() {
    setIsConfigStorageBusy(true);
    try {
      const defaultConfig = await resetConfig();
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

  return (
    <main className="app-shell">
      <Header
        activeView={settingsOpen ? "settings" : "overview"}
        appVersion={appVersion}
        isLoading={isLoading}
        onOpenOverview={() => setSettingsOpen(false)}
        onOpenSettings={() => setSettingsOpen(true)}
        onRefresh={loadSnapshot}
      />
      {settingsOpen && config ? (
        <SettingsPanel
          config={config}
          configStorageInfo={configStorageInfo}
          isConfigStorageBusy={isConfigStorageBusy}
          isSaving={isSaving}
          snapshotProviders={snapshot?.providers ?? []}
          onChange={setConfig}
          onOpenConfigFolder={openConfigFolder}
          onResetConfig={restoreDefaultConfig}
          onSave={persistConfig}
          onSetPortableMode={(enabled) => void togglePortableMode(enabled)}
        />
      ) : (
        <section className="overview-page" aria-label={t.app.overviewLabel} data-testid="overview-page">
          <GlobalStatusStrip
            providers={snapshot?.providers ?? []}
            refreshedAt={snapshot?.refreshedAt ?? null}
            refreshIntervalSeconds={config?.refreshIntervalSeconds}
            lowQuotaWarningThreshold={config?.lowQuotaWarningThreshold}
          />
          <section className="provider-list" aria-label={t.app.providersLabel}>
            {snapshot?.providers.map((provider) => (
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
      )}
    </main>
  );
}
