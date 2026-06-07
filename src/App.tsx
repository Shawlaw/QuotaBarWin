import { useCallback, useEffect, useRef, useState } from "react";
import { Header } from "./components/Header";
import { ProviderCard } from "./components/ProviderCard";
import { SettingsPanel } from "./components/SettingsPanel";
import { Summary } from "./components/Summary";
import { findGlobalLowestWindow } from "./lib/forecast";
import {
  getCachedSnapshot,
  getAppVersion,
  getConfig,
  getProviderPresets,
  listenForRefreshRequests,
  refreshSnapshot,
  saveConfig,
  testProvider
} from "./lib/api";
import type { AppConfig, AppSnapshot, ProviderPreset } from "./types";

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

export function App() {
  const refreshInFlight = useRef(false);
  const [snapshot, setSnapshot] = useState<AppSnapshot | null>(null);
  const [config, setConfig] = useState<AppConfig | null>(null);
  const [appVersion, setAppVersion] = useState<string>("unknown");
  const [presets, setPresets] = useState<ProviderPreset[]>([]);
  const [isLoading, setIsLoading] = useState(false);
  const [isSaving, setIsSaving] = useState(false);
  const [settingsOpen, setSettingsOpen] = useState(false);

  const loadSnapshot = useCallback(async () => {
    if (refreshInFlight.current) {
      return;
    }

    refreshInFlight.current = true;
    setIsLoading(true);
    try {
      setSnapshot(await refreshSnapshot());
    } catch (error) {
      const cached = await getCachedSnapshot();
      setSnapshot(cached ?? fallbackSnapshot(error));
    } finally {
      refreshInFlight.current = false;
      setIsLoading(false);
    }
  }, []);

  useEffect(() => {
    let isMounted = true;

    async function initialize() {
      try {
        const loadedConfig = await getConfig();
        const loadedVersion = await getAppVersion();
        if (!isMounted) {
          return;
        }
        setConfig(loadedConfig);
        setAppVersion(loadedVersion);
        setPresets(await getProviderPresets());
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
    if (!config) {
      return;
    }

    const interval = window.setInterval(
      () => void loadSnapshot(),
      Math.max(10, config.refreshIntervalSeconds) * 1000
    );

    return () => window.clearInterval(interval);
  }, [config, loadSnapshot]);

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    void listenForRefreshRequests(() => void loadSnapshot()).then((cleanup) => {
      unlisten = cleanup;
    });

    return () => {
      unlisten?.();
    };
  }, [loadSnapshot]);

  async function persistConfig() {
    if (!config) {
      return;
    }

    setIsSaving(true);
    try {
      await saveConfig(config);
      setSettingsOpen(false);
      await loadSnapshot();
    } finally {
      setIsSaving(false);
    }
  }

  return (
    <main className="app-shell">
      <Header
        isLoading={isLoading}
        lastRefreshedAt={snapshot?.refreshedAt ?? null}
        onOpenSettings={() => setSettingsOpen(true)}
        onRefresh={loadSnapshot}
      />
      {settingsOpen && config ? (
        <SettingsPanel
          config={config}
          appVersion={appVersion}
          isSaving={isSaving}
          onChange={setConfig}
          onClose={() => setSettingsOpen(false)}
          onSave={persistConfig}
          onTestProvider={testProvider}
          presets={presets}
        />
      ) : null}
      <Summary globalLowest={findGlobalLowestWindow(snapshot)} />
      <section className="provider-list" aria-label="Providers">
        {snapshot?.providers.map((provider) => (
          <ProviderCard
            key={provider.id}
            provider={provider}
            lowQuotaWarningThreshold={config?.lowQuotaWarningThreshold}
          />
        ))}
      </section>
    </main>
  );
}
