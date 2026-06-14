import { useState } from "react";
import type { RemoteProviderConfig } from "../types";
import type { RegistryInstallResult, UpdateInfo } from "../lib/api";
import { useI18n } from "../i18n";

type RemoteProviderSettingsProps = {
  providers: RemoteProviderConfig[];
  onInstallRegistry: (
    url: string,
    proxyUrl: string | null,
    autoUpdate: boolean
  ) => Promise<RegistryInstallResult>;
  onRemove: (id: string) => Promise<void>;
  onRefresh: (id: string) => Promise<UpdateInfo>;
  onCheckUpdates: () => Promise<UpdateInfo[]>;
  onApplyUpdate: (id: string) => Promise<void>;
  onOpenGuide: () => Promise<void>;
};

export function RemoteProviderSettings({
  providers,
  onInstallRegistry,
  onRemove,
  onRefresh,
  onCheckUpdates,
  onApplyUpdate,
  onOpenGuide
}: RemoteProviderSettingsProps) {
  const { t } = useI18n();
  const [url, setUrl] = useState("");
  const [proxyUrl, setProxyUrl] = useState("");
  const [autoUpdate, setAutoUpdate] = useState(true);
  const [loading, setLoading] = useState(false);
  const [message, setMessage] = useState<string | null>(null);
  const [updates, setUpdates] = useState<UpdateInfo[]>([]);

  async function handleInstallRegistry() {
    setMessage(null);
    setLoading(true);
    try {
      const result = await onInstallRegistry(
        url.trim(),
        proxyUrl.trim() || null,
        autoUpdate
      );
      const parts: string[] = [];
      if (result.installed.length > 0) {
        parts.push(t.remoteProviders.installedResult(result.installed.length));
      }
      if (result.skipped.length > 0) {
        parts.push(t.remoteProviders.skippedResult(result.skipped.length));
      }
      if (result.failed.length > 0) {
        parts.push(t.remoteProviders.failedResult(result.failed.length));
      }
      setMessage(parts.join(", ") || t.remoteProviders.noProvidersInstalledFromRegistry);
      setUrl("");
      setProxyUrl("");
      setUpdates([]);
    } catch (error) {
      setMessage(
        error instanceof Error ? error.message : t.remoteProviders.failedToInstallRegistry
      );
    } finally {
      setLoading(false);
    }
  }

  async function handleRemove(id: string) {
    setMessage(null);
    try {
      await onRemove(id);
      setUpdates((current) => current.filter((update) => update.id !== id));
      setMessage(t.remoteProviders.providerRemoved);
    } catch (error) {
      setMessage(error instanceof Error ? error.message : t.remoteProviders.failedToRemoveProvider);
    }
  }

  async function handleRefresh(id: string) {
    setMessage(null);
    try {
      const result = await onRefresh(id);
      if (result.available) {
        setUpdates((current) => [
          ...current.filter((update) => update.id !== id),
          result
        ]);
        setMessage(t.remoteProviders.updateAvailable);
      } else {
        setMessage(t.remoteProviders.providerRefreshed);
      }
    } catch (error) {
      setMessage(error instanceof Error ? error.message : t.remoteProviders.failedToRefreshProvider);
    }
  }

  async function handleCheckUpdates() {
    setMessage(null);
    try {
      const result = await onCheckUpdates();
      setUpdates(result);
      const availableCount = result.filter((update) => update.available).length;
      if (availableCount > 0) {
        setMessage(t.remoteProviders.updatesAvailable(availableCount));
      } else {
        setMessage(t.remoteProviders.allProvidersUpToDate);
      }
    } catch (error) {
      setMessage(error instanceof Error ? error.message : t.remoteProviders.failedToCheckUpdates);
    }
  }

  async function handleApplyUpdate(id: string) {
    setMessage(null);
    try {
      await onApplyUpdate(id);
      setUpdates((current) => current.filter((update) => update.id !== id));
      setMessage(t.remoteProviders.updateApplied);
    } catch (error) {
      setMessage(error instanceof Error ? error.message : t.remoteProviders.failedToApplyUpdate);
    }
  }

  return (
    <section className="settings-section" aria-label={t.remoteProviders.title} data-testid="remote-providers-section">
      <div className="settings-section-title">
        <h3>{t.remoteProviders.title}</h3>
        <div className="settings-section-actions">
          <span>{t.remoteProviders.installedCount(providers.length)}</span>
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

      <div className="settings-grid remote-provider-add-form">
        <label>
          {t.remoteProviders.registryUrl}
          <input
            data-testid="remote-provider-url-input"
            type="text"
            value={url}
            onChange={(event) => setUrl(event.currentTarget.value)}
            placeholder={t.remoteProviders.registryUrlPlaceholder}
          />
        </label>
        <label>
          {t.remoteProviders.providerProxyUrl}
          <input
            data-testid="remote-provider-proxy-url-input"
            type="text"
            value={proxyUrl}
            onChange={(event) => setProxyUrl(event.currentTarget.value)}
            placeholder={t.remoteProviders.providerProxyPlaceholder}
          />
        </label>
        <label className="checkbox-row">
          <input
            type="checkbox"
            checked={autoUpdate}
            onChange={(event) => setAutoUpdate(event.currentTarget.checked)}
          />
          {t.remoteProviders.autoUpdateWhenAvailable}
        </label>
        <button
          type="button"
          className="button-secondary"
          onClick={() => void handleInstallRegistry()}
          disabled={loading || !url.trim()}
          data-testid="install-remote-provider-registry"
        >
          {loading ? t.remoteProviders.loading : t.remoteProviders.installRegistry}
        </button>
      </div>

      {message ? <div className="settings-message">{message}</div> : null}

      {providers.length === 0 ? (
        <p>{t.remoteProviders.noRemoteProvidersInstalled}</p>
      ) : (
        <ul className="remote-provider-list">
          {providers.map((provider) => {
            const update = updates.find((u) => u.id === provider.id);
            return (
              <li key={provider.id} className="remote-provider-item">
                <div className="remote-provider-summary">
                  <strong>{provider.name}</strong>
                  <span>{provider.id}</span>
                  <span>{provider.runtime}</span>
                  {provider.autoUpdate ? <span>{t.remoteProviders.autoUpdate}</span> : null}
                </div>
                <div className="remote-provider-url">{provider.manifestUrl}</div>
                <div className="remote-provider-actions">
                  <button
                    type="button"
                    className="button-secondary"
                    onClick={() => void handleRefresh(provider.id)}
                  >
                    {t.remoteProviders.refresh}
                  </button>
                  <button
                    type="button"
                    className="button-secondary"
                    onClick={() => void handleCheckUpdates()}
                  >
                    {t.remoteProviders.checkUpdates}
                  </button>
                  {update?.available ? (
                    <button
                      type="button"
                      className="button-primary"
                      onClick={() => void handleApplyUpdate(provider.id)}
                    >
                      {t.remoteProviders.applyUpdate}
                    </button>
                  ) : null}
                  <button
                    type="button"
                    className="button-danger"
                    onClick={() => void handleRemove(provider.id)}
                  >
                    {t.remoteProviders.remove}
                  </button>
                </div>
              </li>
            );
          })}
        </ul>
      )}
    </section>
  );
}
