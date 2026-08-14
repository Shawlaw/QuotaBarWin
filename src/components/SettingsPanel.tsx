import { useEffect, useRef, useState } from "react";
import type {
  AppConfig,
  ConfigStorageInfo,
  ProviderSnapshot,
  ProviderSetupTestResult,
  RemoteProviderConfig,
  RemoteProviderManifest,
  RemoteProviderParameter,
  RegistryMigrationResult
} from "../types";
import {
  applyAppUpdate,
  applyRemoteUpdate,
  checkAppUpdate,
  checkRemoteUpdates,
  downloadAppUpdate,
  getConfig,
  getAppUpdateStatus,
  getInstalledRemoteProviderManifest,
  installRemoteProviderManifest,
  migrateRemoteProvidersToRegistry,
  openAppUpdateNotes,
  openRemoteProviderGuide,
  previewRemoteProviderRegistry,
  removeRemoteProvider,
  listenForAppUpdateStatus
} from "../lib/api";
import type { AppUpdateInfo, UpdateInfo } from "../lib/api";
import { DEFAULT_REMOTE_PROVIDER_TIMEOUT_SECONDS } from "../lib/defaults";
import { useI18n } from "../i18n";
import { NetworkProxySettings } from "./NetworkProxySettings";
import {
  ProviderWindowSettings,
  type WindowDisplayPatch
} from "./ProviderWindowSettings";
import { RemoteProviderSettings } from "./RemoteProviderSettings";
import { ProviderSetupPage } from "./provider-setup/ProviderSetupPage";

const LOG_BYTES_PER_MB = 1024 * 1024;
const DEFAULT_LOG_MAX_BYTES = 10 * LOG_BYTES_PER_MB;

type SettingsPanelProps = {
  appVersion: string;
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
  onProviderSetupConfigChanged: (
    config: AppConfig,
    testResult?: ProviderSetupTestResult
  ) => void;
  onRequestClose: () => void;
  closeRequest: number;
  settingsHomeRequest: number;
  appUpdateFocusRequest: number;
  onAppUpdateFocusHandled: () => void;
  initialProviderSettingsView: "main" | "add";
};

type ProviderManifestState = Record<string, RemoteProviderManifest | null>;

type PendingUnsavedAction = {
  action: () => Promise<void>;
  cancel: () => void;
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
  appVersion,
  config,
  configStorageInfo,
  isConfigStorageBusy,
  isSaving,
  snapshotProviders = [],
  onChange,
  onOpenConfigFolder,
  onResetConfig,
  onSave,
  onSetPortableMode,
  onProviderSetupConfigChanged,
  onRequestClose,
  closeRequest,
  settingsHomeRequest,
  appUpdateFocusRequest,
  onAppUpdateFocusHandled,
  initialProviderSettingsView
}: SettingsPanelProps) {
  const { t } = useI18n();
  const [expandedProviders, setExpandedProviders] = useState<Record<string, boolean>>({});
  const [expandedProviderActions, setExpandedProviderActions] = useState<Record<string, boolean>>({});
  const [envVarDrafts, setEnvVarDrafts] = useState<Record<string, string>>({});
  const [updateInfo, setUpdateInfo] = useState<Record<string, UpdateInfo>>({});
  const [providerManifests, setProviderManifests] = useState<ProviderManifestState>({});
  const [remoteMessage, setRemoteMessage] = useState<string | null>(null);
  const [isCheckingProviderUpdates, setIsCheckingProviderUpdates] = useState(false);
  const [isApplyingProviderUpdates, setIsApplyingProviderUpdates] = useState(false);
  const [appUpdateInfo, setAppUpdateInfo] = useState<AppUpdateInfo | null>(null);
  const [appUpdateMessage, setAppUpdateMessage] = useState<string | null>(null);
  const [isAppUpdateBusy, setIsAppUpdateBusy] = useState(false);
  const [saveMessage, setSaveMessage] = useState(t.settings.noChanges);
  const [providerSettingsView, setProviderSettingsView] = useState<"main" | "add" | "sources" | "setup">(
    initialProviderSettingsView
  );
  const [setupProviderId, setSetupProviderId] = useState<string | null>(null);
  const [quotaDataConfirmOpen, setQuotaDataConfirmOpen] = useState(false);
  const [pendingUnsavedAction, setPendingUnsavedAction] = useState<PendingUnsavedAction | null>(null);
  const [providerPendingRemoval, setProviderPendingRemoval] = useState<RemoteProviderConfig | null>(null);
  const [deleteManagedSecrets, setDeleteManagedSecrets] = useState(true);
  // Navigation requests are events. Capture the value observed at mount so a
  // close from a prior settings session cannot be replayed into a newly opened
  // Provider catalog.
  const handledCloseRequestRef = useRef(closeRequest);
  const handledSettingsHomeRequestRef = useRef(0);
  const handledAppUpdateFocusRequestRef = useRef(0);
  const appUpdateSectionRef = useRef<HTMLElement>(null);
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
  const availableUpdateCount = config.providers.filter(
    (provider) => updateInfo[provider.id]?.available
  ).length;

  useEffect(() => {
    let isMounted = true;
    let unlisten: (() => void) | undefined;
    void getAppUpdateStatus()
      .then((info) => {
        if (isMounted && (info.available || info.checkedAt || info.error || !info.configured)) {
          setAppUpdateInfo(info);
        }
      })
      .catch(() => undefined);
    void listenForAppUpdateStatus((status) => {
      if (isMounted) {
        setAppUpdateInfo(status.info);
      }
    }).then((cleanup) => {
      unlisten = cleanup;
    });

    return () => {
      isMounted = false;
      unlisten?.();
    };
  }, []);

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

  async function saveSettings(): Promise<boolean> {
    setSaveMessage(t.settings.saving);
    try {
      const returnToAddProvider = providerSettingsView === "sources";
      await onSave();
      initialConfigRef.current = JSON.stringify(config);
      setSaveMessage(t.settings.saved);
      if (returnToAddProvider) {
        setProviderSettingsView("add");
      }
      return true;
    } catch (error) {
      setSaveMessage(error instanceof Error ? error.message : t.settings.saveFailed);
      return false;
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

  function runWithUnsavedChangesProtection<T>(action: () => Promise<T>): Promise<T | null> {
    if (!hasChanges) {
      return action();
    }

    return new Promise((resolve) => {
      setPendingUnsavedAction({
        action: async () => {
          resolve(await action());
        },
        cancel: () => resolve(null)
      });
    });
  }

  async function saveAndRunPendingAction() {
    const pending = pendingUnsavedAction;
    if (!pending || !canSave) {
      return;
    }
    const saved = await saveSettings();
    if (!saved) {
      return;
    }
    setPendingUnsavedAction(null);
    await pending.action();
  }

  function discardAndRunPendingAction() {
    const pending = pendingUnsavedAction;
    if (!pending) {
      return;
    }
    resetChanges();
    setPendingUnsavedAction(null);
    void pending.action();
  }

  function cancelPendingAction() {
    pendingUnsavedAction?.cancel();
    setPendingUnsavedAction(null);
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

  function openProviderSetup(providerId: string) {
    setSetupProviderId(providerId);
    setProviderSettingsView("setup");
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

  async function handleCheckAllUpdates() {
    setRemoteMessage(null);
    setIsCheckingProviderUpdates(true);
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
      setUpdateInfo({});
      setRemoteMessage(error instanceof Error ? error.message : t.remoteProviders.failedToCheckUpdates);
    } finally {
      setIsCheckingProviderUpdates(false);
    }
  }

  async function handleApplyUpdate(providerId: string) {
    setRemoteMessage(null);
    setIsApplyingProviderUpdates(true);
    try {
      await applyRemoteUpdate(providerId, updateInfo[providerId]?.updateManifestUrl);
      setUpdateInfo((current) => {
        const next = { ...current };
        delete next[providerId];
        return next;
      });
      const updated = await getConfig();
      acceptPersistedConfig(updated);
      await reloadProviderManifest(providerId);
      setRemoteMessage(t.remoteProviders.updateApplied);
    } catch (error) {
      setRemoteMessage(error instanceof Error ? error.message : t.remoteProviders.failedToApplyUpdate);
    } finally {
      setIsApplyingProviderUpdates(false);
    }
  }

  async function handleApplyAllUpdates() {
    const providerIds = config.providers
      .map((provider) => provider.id)
      .filter((id) => updateInfo[id]?.available);
    if (providerIds.length === 0) {
      return;
    }

    setRemoteMessage(null);
    setIsApplyingProviderUpdates(true);
    const failures: string[] = [];
    try {
      for (const providerId of providerIds) {
        try {
          await applyRemoteUpdate(
            providerId,
            updateInfo[providerId]?.updateManifestUrl
          );
        } catch (error) {
          failures.push(error instanceof Error ? error.message : providerId);
        }
      }
      const updated = await getConfig();
      acceptPersistedConfig(updated);
      setUpdateInfo((current) => {
        const next = { ...current };
        for (const providerId of providerIds) {
          delete next[providerId];
        }
        return next;
      });
      await Promise.all(updated.providers.map((provider) => reloadProviderManifest(provider.id)));
      setRemoteMessage(
        failures.length > 0
          ? t.remoteProviders.updateSomeFailed(providerIds.length - failures.length, failures.length)
          : t.remoteProviders.updatesApplied(providerIds.length)
      );
    } catch (error) {
      setRemoteMessage(error instanceof Error ? error.message : t.remoteProviders.failedToApplyUpdate);
    } finally {
      setIsApplyingProviderUpdates(false);
    }
  }

  async function handleRemoveProvider(provider: RemoteProviderConfig) {
    setProviderPendingRemoval(provider);
    setDeleteManagedSecrets(true);
  }

  async function confirmRemoveProvider() {
    const provider = providerPendingRemoval;
    if (!provider) {
      return;
    }
    setRemoteMessage(null);
    try {
      await removeRemoteProvider(provider.id, deleteManagedSecrets);
      const updated = await getConfig();
      acceptPersistedConfig(updated);
      setRemoteMessage(t.remoteProviders.providerRemoved);
      setProviderPendingRemoval(null);
    } catch (error) {
      setRemoteMessage(error instanceof Error ? error.message : t.remoteProviders.failedToRemoveProvider);
    }
  }

  async function handleAppUpdateCheck() {
    setIsAppUpdateBusy(true);
    setAppUpdateMessage(null);
    try {
      const result = await checkAppUpdate();
      setAppUpdateInfo(result);
      setAppUpdateMessage(
        !result.configured
          ? t.appUpdate.unavailable
          : result.available && result.version
            ? t.appUpdate.available(result.version)
            : t.appUpdate.upToDate
      );
    } catch (error) {
      setAppUpdateMessage(error instanceof Error ? error.message : t.appUpdate.failedToCheck);
    } finally {
      setIsAppUpdateBusy(false);
    }
  }

  async function handleDownloadAndApplyAppUpdate() {
    setIsAppUpdateBusy(true);
    setAppUpdateMessage(t.appUpdate.downloading);
    try {
      if (!appUpdateInfo?.available) {
        setAppUpdateMessage(t.appUpdate.upToDate);
        setIsAppUpdateBusy(false);
        return;
      }
      // download_app_update uses the exact candidate retained by the last signed check. Do not
      // recheck here: a newer manifest published after the notice must not swap the selected
      // version beneath the user.
      const downloaded = await downloadAppUpdate();
      setAppUpdateInfo(downloaded);
      await applyAppUpdate();
    } catch (error) {
      setAppUpdateMessage(error instanceof Error ? error.message : t.appUpdate.failedToDownload);
      setIsAppUpdateBusy(false);
    }
  }

  async function handleOpenAppUpdateNotes() {
    const notesUrl = appUpdateInfo?.notesUrl;
    if (!notesUrl) {
      return;
    }

    try {
      await openAppUpdateNotes(notesUrl);
    } catch (error) {
      setAppUpdateMessage(error instanceof Error ? error.message : t.appUpdate.failedToOpenNotes);
    }
  }

  async function handleInstallManifest(
    url: string,
    checksum: string | null,
    proxyUrl: string | null,
    autoUpdate: boolean
  ): Promise<RemoteProviderConfig | null> {
    return runWithUnsavedChangesProtection(async () => {
      const installed = await installRemoteProviderManifest(
        url,
        checksum,
        proxyUrl,
        autoUpdate
      );
      const updated = await getConfig();
      acceptPersistedConfig(updated);
      openProviderSetup(installed.id);
      return installed;
    });
  }

  async function handleMigrateProviderSource(
    url: string,
    proxyUrl: string | null
  ): Promise<RegistryMigrationResult | null> {
    return runWithUnsavedChangesProtection(async () => {
      const result = await migrateRemoteProvidersToRegistry(url, proxyUrl);
      const updated = await getConfig();
      acceptPersistedConfig(updated);
      await Promise.all(updated.providers.map((provider) => reloadProviderManifest(provider.id)));
      return result;
    });
  }

  useEffect(() => {
    if (closeRequest === 0 || closeRequest <= handledCloseRequestRef.current) {
      return;
    }
    handledCloseRequestRef.current = closeRequest;
    if (providerSettingsView === "setup") {
      return;
    }
    void runWithUnsavedChangesProtection(async () => {
      onRequestClose();
    });
    // A close request is an event, rather than state that should be replayed when the draft changes.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [closeRequest, providerSettingsView]);

  useEffect(() => {
    if (
      settingsHomeRequest === 0 ||
      settingsHomeRequest <= handledSettingsHomeRequestRef.current
    ) {
      return;
    }
    handledSettingsHomeRequestRef.current = settingsHomeRequest;
    // The setup page owns its secret-draft leave confirmation. For the
    // catalog/source views, the header's Settings button returns to the main
    // settings page with the same persisted-draft protection as Back.
    if (providerSettingsView === "setup") {
      return;
    }
    void runWithUnsavedChangesProtection(async () => {
      setSetupProviderId(null);
      setProviderSettingsView("main");
    });
    // This numeric prop represents a one-time header navigation event.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [settingsHomeRequest, providerSettingsView]);

  useEffect(() => {
    if (
      appUpdateFocusRequest === 0 ||
      appUpdateFocusRequest <= handledAppUpdateFocusRequestRef.current
    ) {
      return;
    }
    handledAppUpdateFocusRequestRef.current = appUpdateFocusRequest;
    setSetupProviderId(null);
    setProviderSettingsView("main");
  }, [appUpdateFocusRequest]);

  useEffect(() => {
    if (
      appUpdateFocusRequest === 0 ||
      handledAppUpdateFocusRequestRef.current !== appUpdateFocusRequest ||
      providerSettingsView !== "main"
    ) {
      return;
    }
    const frame = window.requestAnimationFrame(() => {
      appUpdateSectionRef.current?.scrollIntoView?.({ behavior: "smooth", block: "start" });
      // The parent owns this cross-view request. A settings panel is unmounted when the user
      // returns to Overview, so acknowledge it only after the requested scroll has run; a later
      // ordinary visit to Settings must not replay this navigation.
      onAppUpdateFocusHandled();
    });
    return () => window.cancelAnimationFrame(frame);
  }, [appUpdateFocusRequest, onAppUpdateFocusHandled, providerSettingsView]);

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

  function renderUnsavedChangesDialog() {
    if (!pendingUnsavedAction) {
      return null;
    }

    return (
      <div className="dialog-overlay" role="presentation">
        <section
          className="dialog"
          role="dialog"
          aria-modal="true"
          aria-labelledby="unsaved-changes-title"
        >
          <h3 id="unsaved-changes-title">{t.settings.unsavedChangesTitle}</h3>
          <p>{t.settings.unsavedChangesPrompt}</p>
          <div className="dialog-actions">
            <button
              type="button"
              className="button-secondary"
              onClick={cancelPendingAction}
              data-testid="cancel-unsaved-changes"
            >
              {t.settings.cancel}
            </button>
            <button
              type="button"
              className="button-danger"
              onClick={discardAndRunPendingAction}
              data-testid="discard-unsaved-changes"
            >
              {t.settings.discardChanges}
            </button>
            <button
              type="button"
              disabled={!canSave}
              onClick={() => void saveAndRunPendingAction()}
              data-testid="save-and-continue-unsaved-changes"
            >
              {t.settings.saveAndContinue}
            </button>
          </div>
        </section>
      </div>
    );
  }

  function renderRemoveProviderDialog() {
    const provider = providerPendingRemoval;
    if (!provider) {
      return null;
    }
    return (
      <div className="dialog-overlay" role="presentation">
        <section className="dialog" role="dialog" aria-modal="true" aria-labelledby="remove-provider-title">
          <h3 id="remove-provider-title">{t.settings.removeProviderConfirm(provider.name)}</h3>
          <label className="checkbox-row settings-toggle-row">
            <input
              type="checkbox"
              data-testid="remove-managed-secrets"
              checked={deleteManagedSecrets}
              onChange={(event) => setDeleteManagedSecrets(event.currentTarget.checked)}
            />
            {t.settings.removeManagedSecrets}
          </label>
          <p className="settings-hint">{t.settings.removeManagedSecretsHint}</p>
          <div className="dialog-actions">
            <button type="button" className="button-secondary" onClick={() => setProviderPendingRemoval(null)}>
              {t.settings.cancel}
            </button>
            <button
              type="button"
              className="button-danger"
              data-testid="confirm-remove-provider"
              onClick={() => void confirmRemoveProvider()}
            >
              {t.settings.remove}
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
          onMigrateSource={handleMigrateProviderSource}
          onOpenGuide={openRemoteProviderGuide}
          onBackToSettings={() => {
            void runWithUnsavedChangesProtection(async () => {
              setProviderSettingsView("main");
            });
          }}
          onBackToAddProvider={() => {
            void runWithUnsavedChangesProtection(async () => {
              setProviderSettingsView("add");
            });
          }}
          onManageSources={() => {
            void runWithUnsavedChangesProtection(async () => {
              setProviderSettingsView("sources");
            });
          }}
        />
        {renderSaveBar()}
        {renderUnsavedChangesDialog()}
      </section>
    );
  }

  if (providerSettingsView === "setup" && setupProviderId) {
    return (
      <section className="settings-panel" aria-label={t.settings.title} data-testid="settings-page">
        <ProviderSetupPage
          providerId={setupProviderId}
          onBack={() => setProviderSettingsView("main")}
          onComplete={() => setProviderSettingsView("main")}
          closeRequest={closeRequest}
          onRequestClose={onRequestClose}
          onConfigChanged={async (testResult) => {
            const updated = await getConfig();
            acceptPersistedConfig(updated);
            onProviderSetupConfigChanged(updated, testResult);
          }}
        />
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
        <section
          className={`settings-section${appUpdateInfo?.available ? " settings-section--app-update-available" : ""}`}
          aria-label={t.appUpdate.title}
          data-testid="app-update-section"
          ref={appUpdateSectionRef}
        >
          <div className="settings-section-title">
            <h3>{t.appUpdate.title}</h3>
            <span>{t.appUpdate.currentVersion(appUpdateInfo?.currentVersion ?? appVersion)}</span>
          </div>
          <div className="settings-field">
            <label className="checkbox-row settings-toggle-row">
              <input
                type="checkbox"
                checked={config.appUpdate?.autoCheck ?? false}
                data-testid="app-update-auto-check"
                onChange={(event) =>
                  onChange({
                    ...config,
                    appUpdate: { autoCheck: event.currentTarget.checked }
                  })
                }
              />
              {t.appUpdate.autoCheck}
            </label>
            <span className="settings-hint">{t.appUpdate.autoCheckHint}</span>
          </div>
          {appUpdateInfo?.available && appUpdateInfo.version ? (
            <div className="app-update-available-state" role="status">
              {t.appUpdate.available(appUpdateInfo.version)}
            </div>
          ) : null}
          <div className="settings-actions settings-actions--inline">
            <button
              type="button"
              className="button-secondary"
              disabled={isAppUpdateBusy}
              onClick={() => void handleAppUpdateCheck()}
            >
              {isAppUpdateBusy ? t.appUpdate.checking : t.appUpdate.check}
            </button>
            {appUpdateInfo?.available && appUpdateInfo.version ? (
              <button
                type="button"
                className="button-primary"
                disabled={isAppUpdateBusy}
                onClick={() => void handleDownloadAndApplyAppUpdate()}
              >
                {isAppUpdateBusy ? t.appUpdate.downloading : t.appUpdate.downloadAndRestart}
              </button>
            ) : null}
            {appUpdateInfo?.notesUrl ? (
              <button
                type="button"
                className="button-secondary"
                disabled={isAppUpdateBusy}
                onClick={() => void handleOpenAppUpdateNotes()}
              >
                {t.appUpdate.notes}
              </button>
            ) : null}
          </div>
          {appUpdateInfo?.checkedAt ? (
            <div className="settings-hint">
              {t.appUpdate.lastChecked(formatProviderDate(appUpdateInfo.checkedAt, appUpdateInfo.checkedAt))}
            </div>
          ) : null}
          {appUpdateInfo?.error ? <div className="settings-message">{t.appUpdate.checkFailed}</div> : null}
          {appUpdateMessage ? <div className="settings-message">{appUpdateMessage}</div> : null}
        </section>
      </section>

      <section className="settings-section" aria-label={t.settings.providers} data-testid="providers-settings-section">
        <div className="settings-section-title">
          <h3>{t.settings.providers}</h3>
          <div className="settings-section-actions">
            <span>{t.settings.configuredCount(config.providers.length)}</span>
            <button
              type="button"
              className="button-primary"
              onClick={() => {
                void runWithUnsavedChangesProtection(async () => {
                  setProviderSettingsView("add");
                });
              }}
            >
              {t.settings.addProvider}
            </button>
            <button
              type="button"
              className="button-secondary"
              disabled={isCheckingProviderUpdates || isApplyingProviderUpdates}
              onClick={() => {
                void runWithUnsavedChangesProtection(handleCheckAllUpdates);
              }}
            >
              {isCheckingProviderUpdates ? t.remoteProviders.checkingUpdates : t.remoteProviders.checkUpdates}
            </button>
            {availableUpdateCount > 0 ? (
              <button
                type="button"
                className="button-primary"
                disabled={isCheckingProviderUpdates || isApplyingProviderUpdates}
                  onClick={() => {
                    void runWithUnsavedChangesProtection(handleApplyAllUpdates);
                  }}
                data-testid="apply-all-provider-updates"
              >
                {isApplyingProviderUpdates
                  ? t.remoteProviders.applyingUpdates
                  : t.remoteProviders.applyAllUpdates(availableUpdateCount)}
              </button>
            ) : null}
          </div>
        </div>
        {remoteMessage ? <div className="settings-message">{remoteMessage}</div> : null}
        <div className="settings-provider-list">
        {config.providers.length === 0 ? (
          <p className="settings-empty">{t.settings.noProviders}</p>
        ) : null}
        {config.providers.map((provider, providerIndex) => {
          const setupState = provider.setupState ?? "ready";
          const needsAttention =
            setupState === "ready" &&
            snapshotProviders.find((snapshotProvider) => snapshotProvider.id === provider.id)?.status === "error";
          const displayedSetupState = needsAttention ? "needs-attention" : setupState;

          return (
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
              <div className="settings-provider__meta">
                <span>{provider.version ?? shortChecksum(provider.trustedChecksum) ?? t.remoteProviders.unknownVersion}</span>
                <span className={`provider-setup__state provider-setup__state--${displayedSetupState}`}>
                  {needsAttention
                    ? t.providerSetup.stateNeedsAttention
                    : setupState === "pending"
                      ? t.providerSetup.statePending
                      : setupState === "unverified"
                        ? t.providerSetup.stateUnverified
                        : t.providerSetup.stateReady}
                </span>
              </div>
              <div className="settings-provider__controls">
                {(setupState === "pending" || setupState === "unverified" || needsAttention) ? (
                  <button
                    type="button"
                    className="button-primary button-compact"
                    onClick={() => openProviderSetup(provider.id)}
                    data-testid={`setup-provider-${provider.id}`}
                  >
                    {needsAttention
                      ? t.providerSetup.repairConfiguration
                      : setupState === "pending"
                        ? t.providerSetup.completeSetup
                        : t.providerSetup.testConfiguration}
                  </button>
                ) : null}
                {updateInfo[provider.id]?.available ? (
                  <button
                    type="button"
                    className="button-primary button-compact"
                    disabled={isCheckingProviderUpdates || isApplyingProviderUpdates}
                    onClick={() => {
                      void runWithUnsavedChangesProtection(() => handleApplyUpdate(provider.id));
                    }}
                    data-testid={`apply-provider-update-${provider.id}`}
                  >
                    {isApplyingProviderUpdates
                      ? t.remoteProviders.applyingUpdates
                      : t.remoteProviders.applyUpdate}
                  </button>
                ) : null}
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
                    void runWithUnsavedChangesProtection(() => handleRemoveProvider(provider));
                  }}
                >
                  {t.settings.remove}
                </button>
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
          );
        })}
        </div>
      </section>

      {renderSaveBar()}
      {renderQuotaDataConfirmDialog()}
      {renderUnsavedChangesDialog()}
      {renderRemoveProviderDialog()}
    </section>
  );
}
