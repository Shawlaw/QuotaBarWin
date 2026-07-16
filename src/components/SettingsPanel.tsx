import { useEffect, useRef, useState } from "react";
import type {
  AppConfig,
  ConfigStorageInfo,
  ProviderSnapshot,
  RemoteProviderConfig,
  RemoteProviderManifest,
  RemoteProviderParameter
} from "../types";
import {
  applyRemoteUpdate,
  checkRemoteUpdates,
  getConfig,
  getInstalledRemoteProviderManifest,
  installRemoteProviderManifest,
  openRemoteProviderGuide,
  previewRemoteProviderRegistry,
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

type ProviderManifestState = Record<string, RemoteProviderManifest | null>;

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

function parameterHintsFromManifest(
  manifest: RemoteProviderManifest | null | undefined
): RemoteProviderParameter[] {
  if (!manifest) {
    return [];
  }

  if (manifest.parameters?.length) {
    return manifest.parameters;
  }

  return (manifest.requiredEnvVars ?? []).map((name) => ({
    name,
    kind: "secret",
    required: true,
    defaultValue: `\${secret:${name}}`
  }));
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
  const [providerManifests, setProviderManifests] = useState<ProviderManifestState>({});
  const [remoteMessage, setRemoteMessage] = useState<string | null>(null);
  const [saveMessage, setSaveMessage] = useState(t.settings.noChanges);
  const [providerSettingsView, setProviderSettingsView] = useState<"main" | "add" | "sources">("main");
  const [quotaDataConfirmOpen, setQuotaDataConfirmOpen] = useState(false);
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

  useEffect(() => {
    const providerIds = config.providers
      .filter((provider) => expandedProviders[provider.id] && !(provider.id in providerManifests))
      .map((provider) => provider.id);
    if (providerIds.length === 0) {
      return;
    }

    let cancelled = false;
    for (const providerId of providerIds) {
      void getInstalledRemoteProviderManifest(providerId)
        .then((manifest) => {
          if (!cancelled) {
            setProviderManifests((current) => ({ ...current, [providerId]: manifest }));
          }
        })
        .catch(() => {
          if (!cancelled) {
            setProviderManifests((current) => ({ ...current, [providerId]: null }));
          }
        });
    }

    return () => {
      cancelled = true;
    };
  }, [config.providers, expandedProviders, providerManifests]);

  useEffect(() => {
    function handleSaveShortcut(event: KeyboardEvent) {
      if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "s") {
        event.preventDefault();
        if (canSave) {
          void saveSettings();
        }
      }
    }

    window.addEventListener("keydown", handleSaveShortcut);
    return () => window.removeEventListener("keydown", handleSaveShortcut);
  });

  async function saveSettings() {
    setSaveMessage(t.settings.saving);
    try {
      const returnToAddProvider = providerSettingsView === "sources";
      await onSave();
      initialConfigRef.current = JSON.stringify(config);
      setSaveMessage(t.settings.saved);
      if (returnToAddProvider) {
        setProviderSettingsView("add");
      }
    } catch (error) {
      setSaveMessage(error instanceof Error ? error.message : t.settings.saveFailed);
    }
  }

  function resetChanges() {
    onChange(JSON.parse(initialConfigRef.current) as AppConfig);
    setEnvVarDrafts({});
    setSaveMessage(t.settings.noChanges);
  }

  function acceptPersistedConfig(updated: AppConfig) {
    initialConfigRef.current = JSON.stringify(updated);
    onChange(updated);
    setSaveMessage(t.settings.saved);
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

  function updateLogQuotaData(enabled: boolean) {
    if (enabled && !(config.logQuotaData ?? false)) {
      setQuotaDataConfirmOpen(true);
      return;
    }

    onChange({
      ...config,
      logQuotaData: enabled
    });
  }

  function enableQuotaDataLogging() {
    setQuotaDataConfirmOpen(false);
    onChange({
      ...config,
      logQuotaData: true
    });
  }

  function snapshotWindowsForProvider(providerId: string) {
    return snapshotProviders.find((provider) => provider.id === providerId)?.windows ?? [];
  }

  function renderProviderParameters(provider: RemoteProviderConfig) {
    const manifest = providerManifests[provider.id];
    const parameters = parameterHintsFromManifest(manifest);
    if (parameters.length === 0) {
      return null;
    }

    return (
      <section className="provider-parameters args-field" aria-label={t.settings.providerParameters}>
        <div className="provider-parameters__header">
          <h4>{t.settings.providerParameters}</h4>
          <span>{t.settings.providerParametersHint}</span>
        </div>
        <div className="provider-parameters__list">
          {parameters.map((parameter) => {
            const details = [
              parameter.kind ? t.settings.parameterKind(parameter.kind) : null,
              parameter.required ? t.settings.parameterRequired : t.settings.optional,
              parameter.defaultValue ? t.settings.parameterDefault(parameter.defaultValue) : null,
              parameter.placeholder ? t.settings.parameterPlaceholder(parameter.placeholder) : null,
              parameter.options?.length ? t.settings.parameterOptions(parameter.options.join(", ")) : null
            ].filter(Boolean);
            return (
              <div className="provider-parameters__row" key={parameter.name}>
                <div>
                  <strong>{parameter.label ?? parameter.name}</strong>
                  {parameter.label ? <code>{parameter.name}</code> : null}
                </div>
                {details.length > 0 ? <span>{details.join(" · ")}</span> : null}
                {parameter.description ? <p>{parameter.description}</p> : null}
              </div>
            );
          })}
        </div>
      </section>
    );
  }

  async function reloadProviderManifest(providerId: string) {
    try {
      const manifest = await getInstalledRemoteProviderManifest(providerId);
      setProviderManifests((current) => ({ ...current, [providerId]: manifest }));
    } catch {
      setProviderManifests((current) => ({ ...current, [providerId]: null }));
    }
  }

  async function handleProviderUpdateCheck(providerId: string) {
    setRemoteMessage(null);
    try {
      const result = await refreshRemoteProvider(providerId);
      setUpdateInfo((current) => ({ ...current, [providerId]: result }));
      setRemoteMessage(
        result.available ? t.remoteProviders.updateAvailable : t.remoteProviders.providerRefreshed
      );
      await reloadProviderManifest(providerId);
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
      acceptPersistedConfig(updated);
      await Promise.all(updated.providers.map((provider) => reloadProviderManifest(provider.id)));
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
      acceptPersistedConfig(updated);
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
      acceptPersistedConfig(updated);
      setRemoteMessage(t.remoteProviders.providerRemoved);
    } catch (error) {
      setRemoteMessage(error instanceof Error ? error.message : t.remoteProviders.failedToRemoveProvider);
    }
  }

  async function handleInstallManifest(
    url: string,
    checksum: string | null,
    proxyUrl: string | null,
    autoUpdate: boolean
  ) {
    const installed = await installRemoteProviderManifest(
      url,
      checksum,
      proxyUrl,
      autoUpdate
    );
    const updated = await getConfig();
    acceptPersistedConfig(updated);
    return installed;
  }

  function renderSaveBar() {
    return (
      <div className="fixed-save-bar" data-testid="fixed-save-bar">
        <span>{hasChanges ? t.settings.unsavedChanges : saveMessage}</span>
        <div className="settings-actions">
          <button type="button" className="button-secondary" onClick={resetChanges} disabled={!hasChanges || isSaving}>
            {t.settings.resetChanges}
          </button>
          <button type="button" onClick={() => void saveSettings()} disabled={!canSave} data-testid="save-settings-button" title={`${t.settings.save} (Ctrl+S)`}>
            {isSaving ? t.settings.saving : t.settings.save}
          </button>
        </div>
      </div>
    );
  }

  function renderQuotaDataConfirmDialog() {
    if (!quotaDataConfirmOpen) {
      return null;
    }

    return (
      <div className="dialog-overlay" role="presentation">
        <section
          className="dialog"
          role="dialog"
          aria-modal="true"
          aria-labelledby="quota-data-confirm-title"
        >
          <h3 id="quota-data-confirm-title">{t.settings.logQuotaDataConfirmTitle}</h3>
          <p>{t.settings.logQuotaDataConfirm}</p>
          <div className="dialog-actions">
            <button
              type="button"
              className="button-secondary"
              onClick={() => setQuotaDataConfirmOpen(false)}
            >
              {t.settings.cancel}
            </button>
            <button type="button" onClick={enableQuotaDataLogging}>
              {t.settings.logQuotaDataConfirmAction}
            </button>
          </div>
        </section>
      </div>
    );
  }

  if (providerSettingsView === "add" || providerSettingsView === "sources") {
    return (
      <section className="settings-panel" aria-label={t.settings.title} data-testid="settings-page">
        <RemoteProviderSettings
          view={providerSettingsView}
          registrySettings={remoteProviderRegistrySettings(config)}
          installedProviderIds={config.providers.map((provider) => provider.id)}
          onRegistrySettingsChange={(remoteProviderRegistry) =>
            onChange({
              ...config,
              remoteProviderRegistry
            })
          }
          onPreviewRegistry={previewRemoteProviderRegistry}
          onInstallManifest={handleInstallManifest}
          onOpenGuide={openRemoteProviderGuide}
          onBackToSettings={() => setProviderSettingsView("main")}
          onBackToAddProvider={() => setProviderSettingsView("add")}
          onManageSources={() => setProviderSettingsView("sources")}
        />
        {renderSaveBar()}
      </section>
    );
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
        <div className="settings-field">
          <label className="checkbox-row settings-toggle-row">
            <input
              type="checkbox"
              checked={config.logQuotaData ?? false}
              onChange={(event) => updateLogQuotaData(event.currentTarget.checked)}
            />
            {t.settings.logQuotaData}
          </label>
          <span className="settings-hint">{t.settings.logQuotaDataHint}</span>
        </div>
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
            <button type="button" className="button-primary" onClick={() => setProviderSettingsView("add")}>
              {t.settings.addProvider}
            </button>
            <button type="button" className="button-secondary" onClick={() => void handleCheckAllUpdates()}>
              {t.remoteProviders.checkUpdates}
            </button>
          </div>
        </div>
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
                    <label className="checkbox-row settings-toggle-row">
                      <input
                        type="checkbox"
                        data-testid={`provider-show-in-tray-${provider.id}`}
                        checked={provider.showInTray !== false}
                        onChange={(event) =>
                          updateRemoteProvider(provider, {
                            showInTray: event.currentTarget.checked
                          })
                        }
                      />
                      {t.settings.showInTray}
                    </label>
                    {renderProviderParameters(provider)}
                    <div className="args-field settings-field">
                      <label htmlFor={`provider-env-vars-${provider.id}`}>
                        {t.settings.remoteEnvVars}
                      </label>
                      <textarea
                        id={`provider-env-vars-${provider.id}`}
                        aria-describedby={`provider-env-vars-hint-${provider.id}`}
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
                      <span
                        id={`provider-env-vars-hint-${provider.id}`}
                        className="settings-hint"
                      >
                        {t.settings.remoteEnvVarsHint}
                      </span>
                    </div>
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

      {renderSaveBar()}
      {renderQuotaDataConfirmDialog()}
    </section>
  );
}
