import { useRef, useState } from "react";
import type {
  AppConfig,
  CodexProviderConfig,
  ConfigStorageInfo,
  ProviderConfig,
  ProviderPreset,
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
import { useI18n } from "../i18n";
import { NetworkProxySettings } from "./NetworkProxySettings";
import {
  ProviderWindowSettings,
  type WindowConfigProvider,
  type WindowDisplayPatch
} from "./ProviderWindowSettings";
import { RemoteProviderSettings } from "./RemoteProviderSettings";

type SettingsPanelProps = {
  config: AppConfig;
  configStorageInfo: ConfigStorageInfo | null;
  isConfigStorageBusy: boolean;
  isSaving: boolean;
  presets: ProviderPreset[];
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
  updater: (provider: ProviderConfig) => ProviderConfig
): AppConfig {
  return {
    ...config,
    providers: config.providers.map((provider) =>
      provider.id === providerId ? updater(provider) : provider
    )
  };
}

function removeProvider(config: AppConfig, providerId: string): AppConfig {
  return {
    ...config,
    providers: config.providers.filter((provider) => provider.id !== providerId)
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

function uniqueProviderId(config: AppConfig, providerId: string): string {
  const existingIds = new Set(config.providers.map((provider) => provider.id));
  if (!existingIds.has(providerId)) {
    return providerId;
  }

  let index = 2;
  while (existingIds.has(`${providerId}-${index}`)) {
    index += 1;
  }

  return `${providerId}-${index}`;
}

function cloneProvider(provider: ProviderConfig): ProviderConfig {
  return JSON.parse(JSON.stringify(provider)) as ProviderConfig;
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

function remoteProviderRegistrySettings(config: AppConfig) {
  return (
    config.remoteProviderRegistry ?? {
      registryUrl: null,
      providerProxyUrl: null,
      autoUpdate: true
    }
  );
}

export function SettingsPanel({
  config,
  configStorageInfo,
  isConfigStorageBusy,
  isSaving,
  presets,
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
  const canSave = hasChanges && !refreshIntervalError && !lowQuotaWarningError && !isSaving;

  function addPreset(preset: ProviderPreset) {
    const provider = cloneProvider(preset.providerConfigTemplate);
    provider.id = uniqueProviderId(config, provider.id);

    setExpandedProviders((current) => ({ ...current, [provider.id]: true }));
    onChange({
      ...config,
      providers: [...config.providers, provider]
    });
  }

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

  function updateCodexProvider(
    provider: CodexProviderConfig,
    patch: Partial<CodexProviderConfig>
  ) {
    onChange(
      updateProvider(config, provider.id, (current) =>
        current.kind === "codex" ? { ...current, ...patch } : current
      )
    );
  }

  function updateWindowConfigProvider(
    provider: WindowConfigProvider,
    patch: WindowDisplayPatch
  ) {
    onChange(
      updateProvider(config, provider.id, (current) =>
        current.kind === "codex" || current.kind === "remote"
          ? { ...current, ...patch }
          : current
      )
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
      updateProvider(config, provider.id, (current) =>
        current.kind === "remote" ? { ...current, ...patch } : current
      )
    );
  }

  function snapshotWindowsForProvider(providerId: string) {
    return snapshotProviders.find((provider) => provider.id === providerId)?.windows ?? [];
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
      </section>

      <section className="settings-section" aria-label={t.settings.providers} data-testid="providers-settings-section">
        <div className="settings-section-title">
          <h3>{t.settings.providers}</h3>
          <span>{t.settings.configuredCount(config.providers.length)}</span>
        </div>
        {presets.length > 0 ? (
          <section className="preset-list" aria-label={t.settings.addProvider}>
            <h3>{t.settings.addProvider}</h3>
            <div className="preset-actions">
              {presets.map((preset) => (
                <button
                  type="button"
                  className="button-secondary"
                  key={preset.id}
                  onClick={() => addPreset(preset)}
                >
                  {preset.displayName}
                </button>
              ))}
            </div>
          </section>
        ) : null}
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
              <span>{provider.kind}</span>
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
                  onClick={() => {
                    if (window.confirm(t.settings.removeProviderConfirm(provider.name))) {
                      onChange(removeProvider(config, provider.id));
                    }
                  }}
                >
                  {t.settings.remove}
                </button>
              </div>
            ) : null}
            {expandedProviders[provider.id] ? (
              <>
                {presets
                  .find((preset) => preset.providerConfigTemplate.id === provider.id)
                  ?.requiredEnvVars?.map((envVar) => (
                    <p className="env-hint" key={envVar}>
                      {t.settings.setEnvVar(envVar)}
                    </p>
                  ))}
                {provider.kind === "codex" ? (
                  <div className="command-fields">
                    <label>
                      {t.settings.name}
                      <input
                        value={provider.name}
                        onChange={(event) =>
                          updateCodexProvider(provider, { name: event.currentTarget.value })
                        }
                      />
                    </label>
                    <label className="args-field">
                      {t.settings.authToken}
                      <textarea
                        rows={3}
                        placeholder={t.settings.authTokenPlaceholder}
                        value={provider.authToken}
                        onChange={(event) =>
                          updateCodexProvider(provider, { authToken: event.currentTarget.value })
                        }
                      />
                    </label>
                    <label>
                      {t.settings.accountId}
                      <input
                        placeholder={t.settings.optional}
                        value={provider.accountId ?? ""}
                        onChange={(event) =>
                          updateCodexProvider(provider, {
                            accountId: event.currentTarget.value || null
                          })
                        }
                      />
                    </label>
                    <label>
                      {t.settings.proxyUrl}
                      <input
                        placeholder={t.settings.providerProxyPlaceholder}
                        value={provider.proxyUrl ?? ""}
                        onChange={(event) =>
                          updateCodexProvider(provider, {
                            proxyUrl: event.currentTarget.value || null
                          })
                        }
                      />
                    </label>
                    <label>
                      {t.settings.timeout}
                      <input
                        type="number"
                        min={100}
                        value={provider.timeoutMs}
                        onChange={(event) =>
                          updateCodexProvider(provider, {
                            timeoutMs: Number(event.currentTarget.value)
                          })
                        }
                      />
                    </label>
                    <ProviderWindowSettings
                      provider={provider}
                      snapshotWindows={snapshotWindowsForProvider(provider.id)}
                      onChange={(patch) => updateWindowConfigProvider(provider, patch)}
                    />
                  </div>
                ) : null}
                {provider.kind === "remote" ? (
                  <div className="command-fields">
                    <label>
                      {t.settings.name}
                      <input
                        value={provider.name}
                        onChange={(event) => updateProviderName(provider.id, event.currentTarget.value)}
                      />
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
                ) : null}
              </>
            ) : null}
          </article>
        ))}
        </div>
      </section>

      <RemoteProviderSettings
        providers={config.providers.filter(
          (provider): provider is RemoteProviderConfig => provider.kind === "remote"
        )}
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
        onRemove={async (id) => {
          await removeRemoteProvider(id);
          const updated = await getConfig();
          onChange(updated);
        }}
        onRefresh={refreshRemoteProvider}
        onCheckUpdates={checkRemoteUpdates}
        onApplyUpdate={async (id) => {
          await applyRemoteUpdate(id);
          const updated = await getConfig();
          onChange(updated);
        }}
        onOpenGuide={openRemoteProviderGuide}
      />

      <details className="settings-advanced" data-testid="advanced-settings-section">
        <summary>{t.settings.advanced}</summary>
        <div className="settings-section">
        <details className="settings-info" aria-label={t.settings.configurationStorage}>
          <summary>
            {t.settings.configurationStorage}
            <span>{configStorageInfo?.mode === "portable" ? t.settings.portableMode : t.settings.appDataMode}</span>
          </summary>
          <div className="config-paths">
            <span>{t.settings.configFile}</span>
            <button
              type="button"
              className="path-chip"
              title={configStorageInfo?.configPath}
              onClick={() => void navigator.clipboard?.writeText(configStorageInfo?.configPath ?? "")}
            >
              {configStorageInfo?.configPath ?? t.settings.loadingConfigPath}
            </button>
            <span>{t.settings.appData}</span>
            <code title={configStorageInfo?.appDataConfigPath}>{configStorageInfo?.appDataConfigPath ?? t.settings.loading}</code>
            <span>{t.settings.portable}</span>
            <code title={configStorageInfo?.portableConfigPath}>{configStorageInfo?.portableConfigPath ?? t.settings.loading}</code>
          </div>
          <label className="checkbox-row">
            <input
              type="checkbox"
              checked={configStorageInfo?.mode === "portable"}
              disabled={!configStorageInfo || isConfigStorageBusy}
              onChange={(event) => onSetPortableMode(event.currentTarget.checked)}
            />
            {t.settings.portableMode}
          </label>
          <div className="settings-hint">
            {t.settings.portableModeHint}
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
        </details>
        </div>
      </details>

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
