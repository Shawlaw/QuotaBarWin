import { useState } from "react";
import type { RemoteProviderRegistrySettings } from "../types";
import type { RegistryInstallResult } from "../lib/api";
import { useI18n } from "../i18n";

type RemoteProviderSettingsProps = {
  registrySettings: RemoteProviderRegistrySettings;
  onRegistrySettingsChange: (settings: RemoteProviderRegistrySettings) => void;
  onInstallRegistry: (
    url: string,
    proxyUrl: string | null,
    autoUpdate: boolean
  ) => Promise<RegistryInstallResult>;
  onOpenGuide: () => Promise<void>;
};

export function RemoteProviderSettings({
  registrySettings,
  onRegistrySettingsChange,
  onInstallRegistry,
  onOpenGuide
}: RemoteProviderSettingsProps) {
  const { t } = useI18n();
  const [loading, setLoading] = useState(false);
  const [message, setMessage] = useState<string | null>(null);
  const registryUrl = registrySettings.registryUrl ?? "";
  const providerProxyUrl = registrySettings.providerProxyUrl ?? "";
  const autoUpdate = registrySettings.autoUpdate;

  function updateRegistrySettings(patch: Partial<RemoteProviderRegistrySettings>) {
    onRegistrySettingsChange({
      ...registrySettings,
      ...patch,
      autoUpdate: patch.autoUpdate ?? registrySettings.autoUpdate
    });
  }

  async function handleInstallRegistry() {
    setMessage(null);
    setLoading(true);
    try {
      const result = await onInstallRegistry(
        registryUrl.trim(),
        providerProxyUrl.trim() || null,
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
    } catch (error) {
      setMessage(
        error instanceof Error ? error.message : t.remoteProviders.failedToInstallRegistry
      );
    } finally {
      setLoading(false);
    }
  }

  return (
    <section className="remote-provider-source" aria-label={t.remoteProviders.title} data-testid="remote-providers-section">
      <div className="settings-section-title">
        <h3>{t.remoteProviders.title}</h3>
        <div className="settings-section-actions">
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
            value={registryUrl}
            onChange={(event) =>
              updateRegistrySettings({
                registryUrl: event.currentTarget.value
              })
            }
            placeholder={t.remoteProviders.registryUrlPlaceholder}
          />
        </label>
        <label>
          {t.remoteProviders.providerProxyUrl}
          <input
            data-testid="remote-provider-proxy-url-input"
            type="text"
            value={providerProxyUrl}
            onChange={(event) =>
              updateRegistrySettings({
                providerProxyUrl: event.currentTarget.value
              })
            }
            placeholder={t.remoteProviders.providerProxyPlaceholder}
          />
        </label>
        <label className="checkbox-row">
          <input
            type="checkbox"
            checked={autoUpdate}
            onChange={(event) =>
              updateRegistrySettings({
                autoUpdate: event.currentTarget.checked
              })
            }
          />
          {t.remoteProviders.autoUpdateWhenAvailable}
        </label>
        <button
          type="button"
          className="button-secondary"
          onClick={() => void handleInstallRegistry()}
          disabled={loading || !registryUrl.trim()}
          data-testid="install-remote-provider-registry"
        >
          {loading ? t.remoteProviders.loading : t.remoteProviders.installRegistry}
        </button>
      </div>

      {message ? <div className="settings-message">{message}</div> : null}
    </section>
  );
}
