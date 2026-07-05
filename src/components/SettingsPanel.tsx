import { useRef, useState } from "react";
import type {
  AppConfig,
  ConfigStorageInfo,
  ProviderSnapshot,
  RemoteProviderConfig
} from "../types";
import {
  applyRemoteUpdate,
  checkRemoteUpdates,
  getConfig,
  installRemoteProviderRegistry,
  openRemoteProviderGuide,
  refreshRemoteProvider,
  removeRemoteProvider
} from "../lib/api";
import { DEFAULT_REMOTE_PROVIDER_TIMEOUT_SECONDS } from "../lib/defaults";
import { useI18n } from "../i18n";
import { NetworkProxySettings } from "./NetworkProxySettings";
import {
  ProviderWindowSettings,
  type WindowDisplayPatch
} from "./ProviderWindowSettings";
import { RemoteProviderSettings } from "./RemoteProviderSettings";

const LOG_BYTES_PER_MB = 1024 * 1024;
const DEFAULT_LOG_MAX_BYTES = 10 * LOG_BYTES_PER_MB;

type SettingsPanelProps = {
  config: AppConfig;
  configStorageInfo: ConfigStorageInfo | null;
  isConfigStorageBusy: boolean;
  isSaving: boolean;
  snapshotProviders?: ProviderSnapshot[];
  onChange: (config: AppConfig) => void;
  onOpenConfigFolder: () => Promise<void>;
  onResetConfig: () => Promise<void>;
  onSave: () => void | Promise<void>;
  onSetPortableMode: (enabled: boolean) => void;
};

function updateProvider(
  config: AppConfig,
  providerId: string,
  updater: (provider: RemoteProviderConfig) => RemoteProviderConfig
): AppConfig {
  return {
    ...config,
    providers: config.providers.map((provider) =>
      provider.id === providerId ? updater(provider) : provider
    )
  };
}

function moveProvider(config: AppConfig, providerId: string, direction: -1 | 1): AppConfig {
  const fromIndex = config.providers.findIndex((provider) => provider.id === providerId);
  const toIndex = fromIndex + direction;

  if (fromIndex === -1 || toIndex < 0 || toIndex >= config.providers.length) {
    return config;
  }

  const providers = [...config.providers];
  const [provider] = providers.splice(fromIndex, 1);
  providers.splice(toIndex, 0, provider);

  return {
    ...config,
    providers
  };
}

function formatEnvVars(envVars: Record<string, string> | undefined): string {
  return Object.entries(envVars ?? {})
    .map(([name, value]) => `${name}=${value}`)
    .join("\n");
}

function parseEnvVarsText(text: string): Record<string, string> {
  const envVars: Record<string, string> = {};
  for (const line of text.split(/\r?\n/)) {
    const trimmed = line.trim();
    if (!trimmed || trimmed.startsWith("#")) {
      continue;
    }

    const separatorIndex = trimmed.indexOf("=");
    if (separatorIndex <= 0) {
      continue;
    }

    const name = trimmed.slice(0, separatorIndex).trim();
    const value = trimmed.slice(separatorIndex + 1).trim();
    if (name) {
      envVars[name] = value;
    }
  }
  return envVars;
}

function providerTimeoutSeconds(provider: RemoteProviderConfig): number {
  return provider.timeoutSeconds ?? DEFAULT_REMOTE_PROVIDER_TIMEOUT_SECONDS;
}

function remoteProviderRegistrySettings(config: AppConfig) {
  return (
    config.remoteProviderRegistry ?? {
      registryUrl: null,
      providerProxyUrl: null,
      autoUpdate: true
    }
  );
}

function formatProviderDate(value: string | null | undefined, fallback: string): string {
  if (!value) {
    return fallback;
  }
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) {
    return value;
  }
  return date.toLocaleString();
}

function shortChecksum(value: string | null | undefined): string | null {
  if (!value) {
    return null;
  }
  return value.length > 18 ? `${value.slice(0, 18)}...` : value;
}

export function SettingsPanel({
  config,
  configStorageInfo,
  isConfigStorageBusy,
  isSaving,
  snapshotProviders = [],
  onChange,
  onOpenConfigFolder,
  onResetConfig,
  onSave,
  onSetPortableMode
}: SettingsPanelProps) {
  const { t } = useI18n();
  const [expandedProviders, setExpandedProviders] = useState<Record<string, boolean>>({});
  const [expandedProviderActions, setExpandedProviderActions] = useState<Record<string, boolean>>({});
  const [envVarDrafts, setEnvVarDrafts] = useState<Record<string, string>>({});
  const [updateInfo, setUpdateInfo] = useState<Record<string, Awaited<ReturnType<typeof refreshRemoteProvider>>>>({});
  const [remoteMessage, setRemoteMessage] = useState<string | null>(null);
  const [saveMessage, setSaveMessage] = useState(t.settings.noChanges);
  const initialConfigRef = useRef(JSON.stringify(config));
  const configDraft = JSON.stringify(config);
  const hasChanges = configDraft !== initialConfigRef.current;
  const refreshIntervalError =
    config.refreshIntervalSeconds > 0 ? null : t.settings.refreshIntervalError;
  const lowQuotaWarningError =
    config.lowQuotaWarningThreshold >= 0 && config.lowQuotaWarningThreshold <= 100
      ? null
      : t.settings.lowQuotaWarningError;
  const logMaxMegabytes = Math.round((config.logMaxBytes ?? DEFAULT_LOG_MAX_BYTES) / LOG_BYTES_PER_MB);
  const logMaxSizeError = logMaxMegabytes >= 1 ? null : t.settings.logMaxSizeError;
  const hasProviderTimeoutError = config.providers.some(
    (provider) => providerTimeoutSeconds(provider) < 1
  );
  const canSave =
    hasChanges &&
    !refreshIntervalError &&
    !lowQuotaWarningError &&
    !logMaxSizeError &&
    !hasProviderTimeoutError &&
    !isSaving;
  const isPortableMode = configStorageInfo?.mode === "portable";
  const storageModeLabel = isPortableMode ? t.settings.portableMode : t.settings.appDataMode;

  async function saveSettings() {
    setSaveMessage(t.settings.saving);
    try {
      await onSave();
      initialConfigRef.current = JSON.stringify(config);
      setSaveMessage(t.settings.saved);
      window.setTimeout(() => setSaveMessage(t.settings.noChanges), 1600);
    } catch (error) {
      setSaveMessage(error instanceof Error ? error.message : t.settings.saveFailed);
    }
  }

  function resetChanges() {
    onChange(JSON.parse(initialConfigRef.current) as AppConfig);
    setEnvVarDrafts({});
    setSaveMessage(t.settings.noChanges);
  }

  function updateWindowConfigProvider(
    provider: RemoteProviderConfig,
    patch: WindowDisplayPatch
  ) {
    onChange(
      updateProvider(config, provider.id, (current) => ({ ...current, ...patch }))
    );
  }

  function updateProviderName(providerId: string, name: string) {
    onChange(
      updateProvider(config, providerId, (current) => ({
        ...current,
        name
      }))
    );
  }

  function updateRemoteProvider(
    provider: RemoteProviderConfig,
    patch: Partial<RemoteProviderConfig>
  ) {
    onChange(
      updateProvider(config, provider.id, (current) => ({ ...current, ...patch }))
    );
  }

  function snapshotWindowsForProvider(providerId: string) {
    return snapshotProviders.find((provider) => provider.id === providerId)?.windows ?? [];
  }

  async function handleProviderUpdateCheck(providerId: string) {
    setRemoteMessage(null);
    try {
      const result = await refreshRemoteProvider(providerId);
      setUpdateInfo((current) => ({ ...current, [providerId]: result }));
      setRemoteMessage(
        result.available ? t.remoteProviders.updateAvailable : t.remoteProviders.providerRefreshed
      );
    } catch (error) {
      setRemoteMessage(error instanceof Error ? error.message : t.remoteProviders.failedToRefreshProvider);
    }
  }

  async function handleCheckAllUpdates() {
    setRemoteMessage(null);
    try {
      const result = await checkRemoteUpdates();
      setUpdateInfo(Object.fromEntries(result.map((update) => [update.id, update])));
      const updated = await getConfig();
      onChange(updated);
      const availableCount = result.filter((update) => update.available).length;
      setRemoteMessage(
        availableCount > 0
          ? t.remoteProviders.updatesAvailable(availableCount)
          : t.remoteProviders.allProvidersUpToDate
      );
    } catch (error) {
      setRemoteMessage(error instanceof Error ? error.message : t.remoteProviders.failedToCheckUpdates);
    }
  }

  async function handleApplyUpdate(providerId: string) {
    setRemoteMessage(null);
    try {
      await applyRemoteUpdate(providerId);
      setUpdateInfo((current) => {
        const next = { ...current };
        delete next[providerId];
        return next;
      });
      const updated = await getConfig();
      onChange(updated);
      setRemoteMessage(t.remoteProviders.updateApplied);
    } catch (error) {
      setRemoteMessage(error instanceof Error ? error.message : t.remoteProviders.failedToApplyUpdate);
    }
  }

  async function handleRemoveProvider(provider: RemoteProviderConfig) {
    if (!window.confirm(t.settings.removeProviderConfirm(provider.name))) {
      return;
    }
    setRemoteMessage(null);
    try {
      await removeRemoteProvider(provider.id);
      const updated = await getConfig();
      onChange(updated);
      setRemoteMessage(t.remoteProviders.providerRemoved);
    } catch (error) {
      setRemoteMessage(error instanceof Error ? error.message : t.remoteProviders.failedToRemoveProvider);
    }
  }

  return (
    <section className="settings-panel" aria-label={t.settings.title} data-testid="settings-page">
      <section className="settings-section" aria-label={t.settings.general} data-testid="general-settings-section">
        <div className="settings-section-title">
          <h3>{t.settings.general}</h3>
        </div>
        <div className="settings-grid general-settings-grid">
        <label>
          {t.settings.refreshInterval}
          <input
            type="number"
            min={1}
            data-testid="refresh-interval-input"
            value={config.refreshIntervalSeconds}
            onChange={(event) =>
              onChange({
                ...config,
                refreshIntervalSeconds: Number(event.currentTarget.value)
              })
            }
          />
          {refreshIntervalError ? <span className="field-error">{refreshIntervalError}</span> : null}
        </label>
        <label>
          {t.settings.displayMode}
          <select
            data-testid="display-mode-select"
            value={config.displayMode}
            onChange={(event) =>
              onChange({
                ...config,
                displayMode: event.currentTarget.value as AppConfig["displayMode"]
              })
            }
          >
            <option value="remaining">{t.settings.displayRemaining}</option>
            <option value="used">{t.settings.displayUsed}</option>
          </select>
        </label>
        <label>
          {t.settings.lowQuotaWarning}
          <input
            type="number"
            min={0}
            max={100}
            data-testid="low-quota-warning-input"
            value={config.lowQuotaWarningThreshold}
            onChange={(event) =>
              onChange({
                ...config,
                lowQuotaWarningThreshold: Number(event.currentTarget.value)
              })
            }
          />
          {lowQuotaWarningError ? <span className="field-error">{lowQuotaWarningError}</span> : null}
        </label>
        <label>
          {t.settings.logLevel}
          <select
            value={config.logLevel ?? "info"}
            onChange={(event) =>
              onChange({
                ...config,
                logLevel: event.currentTarget.value
              })
            }
          >
            <option value="debug">{t.settings.logDebug}</option>
            <option value="info">{t.settings.logInfo}</option>
            <option value="warn">{t.settings.logWarn}</option>
            <option value="error">{t.settings.logError}</option>
          </select>
        </label>
        <label>
          {t.settings.logMaxSize}
          <input
            type="number"
            min={1}
            step={1}
            data-testid="log-max-size-input"
            value={logMaxMegabytes}
            onChange={(event) => {
              const megabytes = Number(event.currentTarget.value);
              onChange({
                ...config,
                logMaxBytes: Math.max(0, megabytes) * LOG_BYTES_PER_MB
              });
            }}
          />
          {logMaxSizeError ? <span className="field-error">{logMaxSizeError}</span> : null}
        </label>
        <label>
          {t.settings.language}
          <select
            data-testid="language-select"
            value={config.language}
            onChange={(event) =>
              onChange({
                ...config,
                language: event.currentTarget.value as AppConfig["language"]
              })
            }
          >
            <option value="system">{t.settings.languageSystem}</option>
            <option value="en">{t.settings.languageEnglish}</option>
            <option value="zh-CN">{t.settings.languageChinese}</option>
          </select>
        </label>
        <NetworkProxySettings
          proxy={config.networkProxy}
          onChange={(proxy) => onChange({ ...config, networkProxy: proxy })}
        />
        <label className="checkbox-row settings-toggle-row">
          <input
            type="checkbox"
            checked={config.launchAtStartup ?? false}
            onChange={(event) =>
              onChange({
                ...config,
                launchAtStartup: event.currentTarget.checked
              })
            }
          />
          {t.settings.launchAtStartup}
        </label>
        </div>
        <section className="settings-section config-storage-section" aria-label={t.settings.configurationStorage}>
          <div className="settings-section-title">
            <h3>{t.settings.configurationStorage}</h3>
            <span>{storageModeLabel}</span>
          </div>
          <div className="settings-grid config-storage-grid">
            <div className="config-path-field">
              <span className="config-path-label">{t.settings.configFile}</span>
              <button
                type="button"
                className="path-chip"
                title={configStorageInfo?.configPath}
                onClick={() => void navigator.clipboard?.writeText(configStorageInfo?.configPath ?? "")}
              >
                {configStorageInfo?.configPath ?? t.settings.loadingConfigPath}
              </button>
            </div>
            {isPortableMode ? (
              <div className="config-path-field">
                <span className="config-path-label">{t.settings.portableMarker}</span>
                <code title={configStorageInfo?.portableMarkerPath}>
                  {configStorageInfo?.portableMarkerPath ?? t.settings.loading}
                </code>
              </div>
            ) : null}
            <div className="config-storage-controls">
              <label className="checkbox-row">
                <input
                  type="checkbox"
                  checked={isPortableMode}
                  disabled={!configStorageInfo || isConfigStorageBusy}
                  onChange={(event) => onSetPortableMode(event.currentTarget.checked)}
                />
                {t.settings.portableMode}
              </label>
              <div className="settings-hint">
                {t.settings.portableModeHint}
              </div>
            </div>
          </div>
          <div className="settings-actions settings-actions--inline">
            <button
              type="button"
              className="button-secondary"
              disabled={!configStorageInfo || isConfigStorageBusy}
              onClick={() => void onOpenConfigFolder()}
            >
              {t.settings.openFolder}
            </button>
            <button
              type="button"
              className="button-danger"
              disabled={isConfigStorageBusy}
              onClick={() => {
                if (window.confirm(t.settings.resetConfigConfirm)) {
                  void onResetConfig();
                }
              }}
            >
              {t.settings.resetConfig}
            </button>
          </div>
        </section>
      </section>

      <section className="settings-section" aria-label={t.settings.providers} data-testid="providers-settings-section">
        <div className="settings-section-title">
          <h3>{t.settings.providers}</h3>
          <div className="settings-section-actions">
            <span>{t.settings.configuredCount(config.providers.length)}</span>
            <button type="button" className="button-secondary" onClick={() => void handleCheckAllUpdates()}>
              {t.remoteProviders.checkUpdates}
            </button>
          </div>
        </div>
        <RemoteProviderSettings
          registrySettings={remoteProviderRegistrySettings(config)}
          onRegistrySettingsChange={(remoteProviderRegistry) =>
            onChange({
              ...config,
              remoteProviderRegistry
            })
          }
          onInstallRegistry={async (url, providerProxyUrl, providerAutoUpdate) => {
            const result = await installRemoteProviderRegistry(
              url,
              providerProxyUrl,
              providerAutoUpdate
            );
            const updated = await getConfig();
            onChange(updated);
            return result;
          }}
          onOpenGuide={openRemoteProviderGuide}
        />
        {remoteMessage ? <div className="settings-message">{remoteMessage}</div> : null}
        <div className="settings-provider-list">
        {config.providers.length === 0 ? (
          <p className="settings-empty">{t.settings.noProviders}</p>
        ) : null}
        {config.providers.map((provider, providerIndex) => (
          <article className="settings-provider" key={provider.id} data-testid={`settings-provider-${provider.id}`}>
            <div className="settings-provider__header">
              <label>
                <input
                  type="checkbox"
                  checked={provider.enabled}
                  onChange={(event) =>
                    onChange(
                      updateProvider(config, provider.id, (current) => ({
                        ...current,
                        enabled: event.currentTarget.checked
                      }))
                    )
                  }
                />
                {provider.name}
              </label>
              <span>{provider.version ?? shortChecksum(provider.trustedChecksum) ?? t.remoteProviders.unknownVersion}</span>
              <button
                type="button"
                className="button-secondary"
                data-testid={`edit-provider-${provider.id}`}
                onClick={() =>
                  setExpandedProviders((current) => ({
                    ...current,
                    [provider.id]: !(current[provider.id] ?? false)
                  }))
                }
              >
                {expandedProviders[provider.id] ? t.settings.collapse : t.settings.edit}
              </button>
              <button
                type="button"
                className="button-ghost"
                data-testid={`more-provider-${provider.id}`}
                onClick={() =>
                  setExpandedProviderActions((current) => ({
                    ...current,
                    [provider.id]: !(current[provider.id] ?? false)
                  }))
                }
              >
                {t.settings.more}
              </button>
            </div>
            {expandedProviderActions[provider.id] ? (
              <div className="settings-provider__actions">
                <button
                  type="button"
                  className="button-secondary button-compact"
                  disabled={providerIndex === 0}
                  onClick={() => onChange(moveProvider(config, provider.id, -1))}
                >
                  {t.settings.up}
                </button>
                <button
                  type="button"
                  className="button-secondary button-compact"
                  disabled={providerIndex === config.providers.length - 1}
                  onClick={() => onChange(moveProvider(config, provider.id, 1))}
                >
                  {t.settings.down}
                </button>
                <button
                  type="button"
                  className="button-danger button-compact"
                  data-testid={`remove-provider-${provider.id}`}
                  onClick={() => void handleRemoveProvider(provider)}
                >
                  {t.settings.remove}
                </button>
                <button
                  type="button"
                  className="button-secondary button-compact"
                  onClick={() => void handleProviderUpdateCheck(provider.id)}
                >
                  {t.remoteProviders.checkUpdates}
                </button>
                {updateInfo[provider.id]?.available ? (
                  <button
                    type="button"
                    className="button-primary button-compact"
                    onClick={() => void handleApplyUpdate(provider.id)}
                  >
                    {t.remoteProviders.applyUpdate}
                  </button>
                ) : null}
              </div>
            ) : null}
            {expandedProviders[provider.id] ? (
              <>
                  <div className="command-fields">
                    <div className="remote-provider-meta">
                      <span>{t.remoteProviders.version}: {provider.version ?? shortChecksum(provider.trustedChecksum) ?? t.remoteProviders.unknownVersion}</span>
                      <span>{t.remoteProviders.installedAt}: {formatProviderDate(provider.installedAt, "-")}</span>
                      <span>{t.remoteProviders.updatedAt}: {formatProviderDate(provider.updatedAt, "-")}</span>
                      <span>{t.remoteProviders.lastCheckedAt}: {formatProviderDate(provider.lastCheckedAt, "-")}</span>
                      <span>{t.remoteProviders.runtime}: {provider.runtime}</span>
                      <span title={provider.manifestUrl}>{t.remoteProviders.manifestUrl}: {provider.manifestUrl}</span>
                    </div>
                    <label>
                      {t.settings.name}
                      <input
                        value={provider.name}
                        onChange={(event) => updateProviderName(provider.id, event.currentTarget.value)}
                      />
                    </label>
                    <label>
                      {t.settings.timeout}
                      <input
                        type="number"
                        min={1}
                        step={1}
                        data-testid={`provider-timeout-${provider.id}`}
                        value={providerTimeoutSeconds(provider)}
                        onChange={(event) => {
                          const value = Number(event.currentTarget.value);
                          updateRemoteProvider(provider, {
                            timeoutSeconds: Number.isFinite(value) ? Math.max(0, Math.trunc(value)) : 0
                          });
                        }}
                      />
                      {providerTimeoutSeconds(provider) < 1 ? (
                        <span className="field-error">{t.settings.timeoutError}</span>
                      ) : null}
                    </label>
                    <label className="args-field">
                      {t.settings.remoteEnvVars}
                      <textarea
                        rows={5}
                        placeholder={t.settings.remoteEnvVarsPlaceholder}
                        value={envVarDrafts[provider.id] ?? formatEnvVars(provider.envVars)}
                        onChange={(event) => {
                          const text = event.currentTarget.value;
                          setEnvVarDrafts((current) => ({
                            ...current,
                            [provider.id]: text
                          }));
                          updateRemoteProvider(provider, {
                            envVars: parseEnvVarsText(text)
                          });
                        }}
                      />
                    </label>
                    <ProviderWindowSettings
                      provider={provider}
                      snapshotWindows={snapshotWindowsForProvider(provider.id)}
                      onChange={(patch) => updateWindowConfigProvider(provider, patch)}
                    />
                  </div>
              </>
            ) : null}
          </article>
        ))}
        </div>
      </section>

      <div className="fixed-save-bar" data-testid="fixed-save-bar">
        <span>{hasChanges ? t.settings.unsavedChanges : saveMessage}</span>
        <div className="settings-actions">
          <button type="button" className="button-secondary" onClick={resetChanges} disabled={!hasChanges || isSaving}>
            {t.settings.resetChanges}
          </button>
          <button type="button" onClick={() => void saveSettings()} disabled={!canSave} data-testid="save-settings-button">
            {isSaving ? t.settings.saving : t.settings.save}
          </button>
        </div>
      </div>
    </section>
  );
}
