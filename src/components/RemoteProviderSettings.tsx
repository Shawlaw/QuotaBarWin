import { useEffect, useMemo, useState } from "react";
import type {
  RemoteProviderCatalogEntry,
  RemoteProviderConfig,
  RemoteProviderRegistrySettings,
  RemoteProviderRegistrySource
} from "../types";
import { DEFAULT_REMOTE_PROVIDER_REGISTRY_URL } from "../lib/defaults";
import { useI18n } from "../i18n";

type RemoteProviderSettingsView = "add" | "sources";

type SourceCatalogEntry = RemoteProviderCatalogEntry & {
  sourceId: string;
  sourceName: string;
  sourceProxyUrl: string | null;
  sourceAutoUpdate: boolean;
};

type RemoteProviderSettingsProps = {
  view: RemoteProviderSettingsView;
  registrySettings: RemoteProviderRegistrySettings;
  installedProviderIds: string[];
  onRegistrySettingsChange: (settings: RemoteProviderRegistrySettings) => void;
  onPreviewRegistry: (
    url: string,
    proxyUrl: string | null
  ) => Promise<RemoteProviderCatalogEntry[]>;
  onInstallManifest: (
    url: string,
    checksum: string | null,
    proxyUrl: string | null,
    autoUpdate: boolean
  ) => Promise<RemoteProviderConfig>;
  onOpenGuide: () => Promise<void>;
  onBackToSettings: () => void;
  onBackToAddProvider: () => void;
  onManageSources: () => void;
};

function normalizeSources(
  registrySettings: RemoteProviderRegistrySettings,
  officialSourceName: string
): RemoteProviderRegistrySource[] {
  if (registrySettings.sources?.length) {
    return registrySettings.sources.map((source, index) => ({
      id: source.id || `source-${index + 1}`,
      name: source.name || `${officialSourceName} ${index + 1}`,
      url: source.url ?? "",
      providerProxyUrl: source.providerProxyUrl ?? null,
      autoUpdate: source.autoUpdate ?? registrySettings.autoUpdate,
      enabled: source.enabled ?? true
    }));
  }

  return [
    {
      id: "official",
      name: officialSourceName,
      url: registrySettings.registryUrl ?? DEFAULT_REMOTE_PROVIDER_REGISTRY_URL,
      providerProxyUrl: registrySettings.providerProxyUrl ?? null,
      autoUpdate: registrySettings.autoUpdate,
      enabled: true
    }
  ];
}

function sourceProxy(source: RemoteProviderRegistrySource): string | null {
  return source.providerProxyUrl?.trim() || null;
}

function sourceAutoUpdate(source: RemoteProviderRegistrySource): boolean {
  return source.autoUpdate ?? true;
}

function sourceFingerprint(sources: RemoteProviderRegistrySource[]): string {
  return JSON.stringify(
    sources.map((source) => ({
      id: source.id,
      url: source.url,
      proxy: source.providerProxyUrl ?? null,
      enabled: source.enabled
    }))
  );
}

export function RemoteProviderSettings({
  view,
  registrySettings,
  installedProviderIds,
  onRegistrySettingsChange,
  onPreviewRegistry,
  onInstallManifest,
  onOpenGuide,
  onBackToSettings,
  onBackToAddProvider,
  onManageSources
}: RemoteProviderSettingsProps) {
  const { t } = useI18n();
  const sources = useMemo(
    () => normalizeSources(registrySettings, t.remoteProviders.officialSource),
    [registrySettings, t.remoteProviders.officialSource]
  );
  const enabledSources = sources.filter(
    (source) => source.enabled && source.url.trim()
  );
  const defaultInstallSource = enabledSources[0] ?? sources[0];
  const [catalog, setCatalog] = useState<SourceCatalogEntry[]>([]);
  const [catalogLoading, setCatalogLoading] = useState(false);
  const [catalogMessage, setCatalogMessage] = useState<string | null>(null);
  const [installingKey, setInstallingKey] = useState<string | null>(null);
  const [manifestUrl, setManifestUrl] = useState("");
  const [manifestChecksum, setManifestChecksum] = useState("");
  const [directInstallLoading, setDirectInstallLoading] = useState(false);
  const [installMessage, setInstallMessage] = useState<string | null>(null);
  const catalogSourceFingerprint = sourceFingerprint(sources);

  function persistSources(nextSources: RemoteProviderRegistrySource[]) {
    const primary = nextSources[0];
    onRegistrySettingsChange({
      ...registrySettings,
      registryUrl: primary?.url ?? null,
      providerProxyUrl: primary?.providerProxyUrl ?? null,
      autoUpdate: primary?.autoUpdate ?? registrySettings.autoUpdate,
      sources: nextSources
    });
  }

  function updateSource(
    sourceId: string,
    patch: Partial<RemoteProviderRegistrySource>
  ) {
    persistSources(
      sources.map((source) =>
        source.id === sourceId ? { ...source, ...patch } : source
      )
    );
  }

  function addSource() {
    persistSources([
      ...sources,
      {
        id: `source-${Date.now().toString(36)}`,
        name: `${t.remoteProviders.customSource} ${sources.length + 1}`,
        url: "",
        providerProxyUrl: null,
        autoUpdate: registrySettings.autoUpdate,
        enabled: true
      }
    ]);
  }

  function removeSource(sourceId: string) {
    if (sources.length <= 1) {
      return;
    }
    persistSources(sources.filter((source) => source.id !== sourceId));
  }

  async function loadCatalog() {
    if (enabledSources.length === 0) {
      setCatalog([]);
      setCatalogMessage(t.remoteProviders.noCatalogProviders);
      return;
    }

    setCatalogLoading(true);
    setCatalogMessage(null);
    try {
      const sourceResults = await Promise.all(
        enabledSources.map(async (source) => {
          try {
            const entries = await onPreviewRegistry(
              source.url.trim(),
              sourceProxy(source)
            );
            return { source, entries, error: null as string | null };
          } catch (error) {
            return {
              source,
              entries: [] as RemoteProviderCatalogEntry[],
              error:
                error instanceof Error
                  ? error.message
                  : t.remoteProviders.catalogLoadFailed
            };
          }
        })
      );

      const loadErrors = sourceResults
        .filter((result) => result.error)
        .map((result) =>
          t.remoteProviders.sourceLoadError(
            result.source.name || result.source.url,
            result.error ?? t.remoteProviders.catalogLoadFailed
          )
        );
      const loadedEntries: SourceCatalogEntry[] = sourceResults.flatMap(
        ({ source, entries }) =>
          entries.map((entry) => ({
            ...entry,
            sourceId: source.id,
            sourceName: source.name || source.url,
            sourceProxyUrl: sourceProxy(source),
            sourceAutoUpdate: sourceAutoUpdate(source)
          }))
      );
      const sourceNamesByProviderId = new Map<string, Set<string>>();
      for (const entry of loadedEntries) {
        const names = sourceNamesByProviderId.get(entry.id) ?? new Set<string>();
        names.add(entry.sourceName);
        sourceNamesByProviderId.set(entry.id, names);
      }
      const entriesWithConflicts = loadedEntries.map((entry) => {
        const sourceNames = Array.from(
          sourceNamesByProviderId.get(entry.id) ?? []
        );
        if (sourceNames.length <= 1) {
          return entry;
        }
        return {
          ...entry,
          error:
            entry.error ??
            t.remoteProviders.sourceConflict(entry.id, sourceNames.join(", "))
        };
      });

      setCatalog(entriesWithConflicts);
      setCatalogMessage(
        [
          ...loadErrors,
          entriesWithConflicts.length === 0
            ? t.remoteProviders.noCatalogProviders
            : null
        ]
          .filter(Boolean)
          .join(" · ") || null
      );
    } finally {
      setCatalogLoading(false);
    }
  }

  useEffect(() => {
    if (view === "add") {
      void loadCatalog();
    }
    // Loading is intentionally tied to source fields, so returning from source
    // management reflects the latest draft without an extra click.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [view, catalogSourceFingerprint]);

  async function installFromCatalog(entry: SourceCatalogEntry) {
    setInstallMessage(null);
    setInstallingKey(`${entry.sourceId}:${entry.id}`);
    try {
      await onInstallManifest(
        entry.providerUrl,
        entry.checksum ?? null,
        entry.sourceProxyUrl,
        entry.sourceAutoUpdate
      );
      setCatalog((current) =>
        current.map((item) =>
          item.id === entry.id ? { ...item, installed: true } : item
        )
      );
      setInstallMessage(t.remoteProviders.providerInstalled);
    } catch (error) {
      setInstallMessage(
        error instanceof Error ? error.message : t.remoteProviders.installProviderFailed
      );
    } finally {
      setInstallingKey(null);
    }
  }

  async function installDirectManifest() {
    const trimmedUrl = manifestUrl.trim();
    if (!trimmedUrl) {
      return;
    }

    setInstallMessage(null);
    setDirectInstallLoading(true);
    try {
      await onInstallManifest(
        trimmedUrl,
        manifestChecksum.trim() || null,
        defaultInstallSource ? sourceProxy(defaultInstallSource) : null,
        defaultInstallSource ? sourceAutoUpdate(defaultInstallSource) : true
      );
      setManifestUrl("");
      setManifestChecksum("");
      setInstallMessage(t.remoteProviders.providerInstalled);
      await loadCatalog();
    } catch (error) {
      setInstallMessage(
        error instanceof Error ? error.message : t.remoteProviders.installProviderFailed
      );
    } finally {
      setDirectInstallLoading(false);
    }
  }

  if (view === "sources") {
    return (
      <section
        className="provider-subpage"
        aria-label={t.remoteProviders.manageSourcesTitle}
        data-testid="provider-source-page"
      >
        <div className="settings-section-title provider-subpage__title">
          <h3>{t.remoteProviders.manageSourcesTitle}</h3>
          <div className="settings-section-actions">
            <button
              type="button"
              className="button-secondary"
              onClick={onBackToAddProvider}
            >
              {t.remoteProviders.backToAddProvider}
            </button>
            <button
              type="button"
              className="button-secondary"
              onClick={() => void onOpenGuide()}
              data-testid="open-remote-provider-guide"
            >
              {t.remoteProviders.openGuide}
            </button>
          </div>
        </div>

        <section className="remote-provider-source">
          <div className="settings-section-title">
            <h3>{t.remoteProviders.sourceSettings}</h3>
            <button
              type="button"
              className="button-secondary"
              onClick={addSource}
              data-testid="add-provider-source"
            >
              {t.remoteProviders.addSource}
            </button>
          </div>
          <div className="provider-source-list">
            {sources.map((source, index) => (
              <article className="remote-provider-item" key={source.id}>
                <div className="settings-grid provider-source-form">
                  <label>
                    {t.remoteProviders.sourceName}
                    <input
                      data-testid={`provider-source-name-${source.id}`}
                      type="text"
                      value={source.name}
                      onChange={(event) =>
                        updateSource(source.id, {
                          name: event.currentTarget.value
                        })
                      }
                    />
                  </label>
                  <label>
                    {t.remoteProviders.sourceUrl}
                    <input
                      data-testid={
                        index === 0
                          ? "remote-provider-url-input"
                          : `provider-source-url-${source.id}`
                      }
                      type="text"
                      value={source.url}
                      onChange={(event) =>
                        updateSource(source.id, {
                          url: event.currentTarget.value
                        })
                      }
                      placeholder={t.remoteProviders.registryUrlPlaceholder}
                    />
                  </label>
                  <label>
                    {t.remoteProviders.providerProxyUrl}
                    <input
                      data-testid={
                        index === 0
                          ? "remote-provider-proxy-url-input"
                          : `provider-source-proxy-${source.id}`
                      }
                      type="text"
                      value={source.providerProxyUrl ?? ""}
                      onChange={(event) =>
                        updateSource(source.id, {
                          providerProxyUrl: event.currentTarget.value
                        })
                      }
                      placeholder={t.remoteProviders.providerProxyPlaceholder}
                    />
                  </label>
                  <div className="provider-source-toggles">
                    <label className="checkbox-row settings-toggle-row">
                      <input
                        type="checkbox"
                        checked={source.enabled}
                        onChange={(event) =>
                          updateSource(source.id, {
                            enabled: event.currentTarget.checked
                          })
                        }
                      />
                      {t.remoteProviders.sourceEnabled}
                    </label>
                    <label className="checkbox-row settings-toggle-row">
                      <input
                        type="checkbox"
                        checked={sourceAutoUpdate(source)}
                        onChange={(event) =>
                          updateSource(source.id, {
                            autoUpdate: event.currentTarget.checked
                          })
                        }
                      />
                      {t.remoteProviders.autoUpdateWhenAvailable}
                    </label>
                  </div>
                </div>
                <div className="remote-provider-actions">
                  <button
                    type="button"
                    className="button-danger button-compact"
                    onClick={() => removeSource(source.id)}
                    disabled={sources.length <= 1}
                  >
                    {t.remoteProviders.removeSource}
                  </button>
                </div>
              </article>
            ))}
          </div>
        </section>
      </section>
    );
  }

  return (
    <section
      className="provider-subpage"
      aria-label={t.remoteProviders.addTitle}
      data-testid="add-provider-page"
    >
      <div className="settings-section-title provider-subpage__title">
        <h3>{t.remoteProviders.addTitle}</h3>
        <div className="settings-section-actions">
          <button
            type="button"
            className="button-secondary"
            onClick={onBackToSettings}
          >
            {t.remoteProviders.backToSettings}
          </button>
          <button
            type="button"
            className="button-secondary"
            onClick={() => void onOpenGuide()}
            data-testid="open-remote-provider-guide"
          >
            {t.remoteProviders.openGuide}
          </button>
        </div>
      </div>

      <section className="remote-provider-source">
        <div className="settings-section-title">
          <h3>{t.remoteProviders.recommendedProviders}</h3>
          <div className="settings-section-actions">
            <button
              type="button"
              className="button-secondary"
              onClick={() => void loadCatalog()}
              disabled={catalogLoading}
            >
              {catalogLoading ? t.remoteProviders.loading : t.remoteProviders.refreshCatalog}
            </button>
            <button
              type="button"
              className="button-secondary"
              onClick={onManageSources}
            >
              {t.remoteProviders.manageSources}
            </button>
          </div>
        </div>

        <div className="source-summary">
          <span>{t.remoteProviders.sourceSummary}</span>
          <code title={enabledSources.map((source) => source.url).join("\n")}>
            {t.remoteProviders.enabledSourcesCount(enabledSources.length)}
            {enabledSources.length > 0
              ? ` · ${enabledSources.map((source) => source.name).join(", ")}`
              : ""}
          </code>
        </div>

        {catalogMessage ? (
          <div className="settings-message">{catalogMessage}</div>
        ) : null}
        {installMessage ? (
          <div className="settings-message">{installMessage}</div>
        ) : null}

        <ul className="remote-provider-list provider-catalog-list">
          {catalog.map((entry) => {
            const isInstalled =
              entry.installed || installedProviderIds.includes(entry.id);
            const installKey = `${entry.sourceId}:${entry.id}`;
            const disabled =
              isInstalled || Boolean(entry.error) || installingKey === installKey;

            return (
              <li className="remote-provider-item" key={`${entry.sourceId}:${entry.providerUrl}`}>
                <div className="remote-provider-summary">
                  <div className="remote-provider-identity">
                    <strong>{entry.displayName}</strong>
                    <span>{entry.description ?? entry.id}</span>
                  </div>
                  <div className="remote-provider-badges">
                    <span>{entry.sourceName}</span>
                    {entry.version ? <span>{entry.version}</span> : null}
                    <span>{isInstalled ? t.remoteProviders.installed : entry.id}</span>
                  </div>
                  <button
                    type="button"
                    className="button-secondary button-compact"
                    disabled={disabled}
                    onClick={() => void installFromCatalog(entry)}
                  >
                    {isInstalled
                      ? t.remoteProviders.installed
                      : installingKey === installKey
                        ? t.remoteProviders.loading
                        : t.remoteProviders.installProvider}
                  </button>
                </div>
                {entry.error ? (
                  <div className="field-error">{entry.error}</div>
                ) : null}
              </li>
            );
          })}
        </ul>
      </section>

      <section className="remote-provider-source">
        <div className="settings-section-title">
          <h3>{t.remoteProviders.customInstall}</h3>
        </div>
        <div className="settings-grid provider-manifest-form">
          <label>
            {t.remoteProviders.manifestInstallUrl}
            <input
              data-testid="remote-provider-manifest-url-input"
              type="text"
              value={manifestUrl}
              onChange={(event) => setManifestUrl(event.currentTarget.value)}
              placeholder={t.remoteProviders.manifestUrlPlaceholder}
            />
          </label>
          <label>
            {t.remoteProviders.manifestChecksum}
            <input
              data-testid="remote-provider-manifest-checksum-input"
              type="text"
              value={manifestChecksum}
              onChange={(event) => setManifestChecksum(event.currentTarget.value)}
              placeholder={t.remoteProviders.manifestChecksumPlaceholder}
            />
          </label>
          <button
            type="button"
            className="button-secondary"
            onClick={() => void installDirectManifest()}
            disabled={directInstallLoading || !manifestUrl.trim()}
            data-testid="install-remote-provider-manifest"
          >
            {directInstallLoading
              ? t.remoteProviders.loading
              : t.remoteProviders.installFromManifest}
          </button>
        </div>
      </section>
    </section>
  );
}
