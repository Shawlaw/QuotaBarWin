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
import { LocalApiSettings } from "./LocalApiSettings";
import { NotificationSettingsSection } from "./NotificationSettingsSection";
import { SecretSecuritySettings } from "./SecretSecuritySettings";
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
  onPersistedConfigChanged: (
    config: AppConfig,
    testResult?: ProviderSetupTestResult
  ) => void;
  onRequestClose: () => void;
  closeRequest: number;
  // Why the latest closeRequest was raised, so the unsaved-changes dialog can
  // use wording that matches the flow (tab switch vs hiding the window).
  closeRequestReason?: "navigate" | "hide-window";
  settingsHomeRequest: number;
  appUpdateFocusRequest: number;
  onAppUpdateFocusHandled: () => void;
  onAppUpdateStatusChange: (info: AppUpdateInfo) => void;
  initialProviderSettingsView: "main" | "add";
};

type ProviderManifestState = Record<string, RemoteProviderManifest | null>;

// Top-level settings categories rendered by the left navigation. The state is
// deliberately local to SettingsPanel: the config draft, validation, and save
// flow stay owned by App.tsx / SettingsPanel as a whole.
type SettingsCategory = "providers" | "general" | "notifications" | "app-update" | "advanced";

const SETTINGS_CATEGORY_ORDER: SettingsCategory[] = [
  "providers",
  "general",
  "notifications",
  "app-update",
  "advanced"
];

type PendingUnsavedAction = {
  action: () => Promise<void>;
  cancel: () => void;
  // "hide-window" swaps the dialog copy for the window-close flow (the window
  // only hides to the tray); "navigate" keeps the in-app navigation wording.
  variant: "navigate" | "hide-window";
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
  onPersistedConfigChanged,
  onRequestClose,
  closeRequest,
  closeRequestReason = "navigate",
  settingsHomeRequest,
  appUpdateFocusRequest,
  onAppUpdateFocusHandled,
  onAppUpdateStatusChange,
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
  const [localApiStatusRevision, setLocalApiStatusRevision] = useState(0);
  const [localApiTokenRequired, setLocalApiTokenRequired] = useState(false);
  const [providerSettingsView, setProviderSettingsView] = useState<"main" | "add" | "sources" | "setup">(
    initialProviderSettingsView
  );
  const [settingsCategory, setSettingsCategory] = useState<SettingsCategory>("providers");
  const [setupProviderId, setSetupProviderId] = useState<string | null>(null);
  const [quotaDataConfirmOpen, setQuotaDataConfirmOpen] = useState(false);
  const [pendingUnsavedAction, setPendingUnsavedAction] = useState<PendingUnsavedAction | null>(null);
  const [providerPendingRemoval, setProviderPendingRemoval] = useState<RemoteProviderConfig | null>(null);
  const [deleteManagedSecrets, setDeleteManagedSecrets] = useState(true);
  // Navigation requests are events. Capture the value observed at mount so a
  // close from a prior settings session cannot be replayed into a newly opened
  // Provider catalog.
  const handledCloseRequestRef = useRef(closeRequest);
  // Read through a ref inside the close-request effect: the effect intentionally
  // runs only on closeRequest changes, so a stale closure must not freeze the
  // reason of the latest request.
  const closeRequestReasonRef = useRef(closeRequestReason);
  closeRequestReasonRef.current = closeRequestReason;
  const handledSettingsHomeRequestRef = useRef(0);
  const handledAppUpdateFocusRequestRef = useRef(0);
  const appUpdateSectionRef = useRef<HTMLElement>(null);
  const settingsContentRef = useRef<HTMLDivElement>(null);
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
    !localApiTokenRequired &&
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
      setLocalApiStatusRevision((revision) => revision + 1);
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

  function acceptPersistedConfig(
    updated: AppConfig,
    testResult?: ProviderSetupTestResult,
  ) {
    initialConfigRef.current = JSON.stringify(updated);
    onPersistedConfigChanged(updated, testResult);
    setSaveMessage(t.settings.saved);
  }

  function runWithUnsavedChangesProtection<T>(
    action: () => Promise<T>,
    variant: PendingUnsavedAction["variant"] = "navigate",
  ): Promise<T | null> {
    if (!hasChanges) {
      return action();
    }

    return new Promise((resolve) => {
      setPendingUnsavedAction({
        action: async () => {
          resolve(await action());
        },
        cancel: () => resolve(null),
        variant,
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
      // The main renderer owns the persistent update notice. A command result is
      // authoritative for the window that initiated the check, so update it
      // directly instead of waiting for a cross-webview event to round-trip.
      onAppUpdateStatusChange(result);
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
    }, closeRequestReasonRef.current);
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
      setSettingsCategory("providers");
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
    // The update section lives in its own category now, so switching the view
    // alone is not enough: the category must be active for the section (and
    // its ref) to exist before the follow-up effect can scroll to it.
    setSettingsCategory("app-update");
  }, [appUpdateFocusRequest]);

  useEffect(() => {
    if (
      appUpdateFocusRequest === 0 ||
      handledAppUpdateFocusRequestRef.current !== appUpdateFocusRequest ||
      providerSettingsView !== "main" ||
      settingsCategory !== "app-update"
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
  }, [appUpdateFocusRequest, onAppUpdateFocusHandled, providerSettingsView, settingsCategory]);

  function settingsCategoryLabel(category: SettingsCategory): string {
    switch (category) {
      case "providers":
        return t.settings.providers;
      case "general":
        return t.settings.general;
      case "notifications":
        return t.notificationSettings.title;
      case "app-update":
        return t.appUpdate.title;
      case "advanced":
        return t.settings.categoryAdvanced;
    }
  }

  function switchSettingsCategory(category: SettingsCategory) {
    if (category === settingsCategory) {
      return;
    }
    setSettingsCategory(category);
    // Category content starts at the top; the scroll container is owned by
    // App.tsx, so reset it through the DOM instead of lifting state.
    const scrollRegion = settingsContentRef.current?.closest(".app-view-scroll-region");
    if (scrollRegion instanceof HTMLElement) {
      scrollRegion.scrollTop = 0;
    }
  }

  function handleSettingsNavKeyDown(event: React.KeyboardEvent<HTMLElement>) {
    if (event.key !== "ArrowDown" && event.key !== "ArrowUp") {
      return;
    }
    const buttons = Array.from(
      event.currentTarget.querySelectorAll<HTMLButtonElement>(".settings-nav__button"),
    );
    const currentIndex = buttons.findIndex((button) => button === document.activeElement);
    if (currentIndex === -1) {
      return;
    }
    event.preventDefault();
    const offset = event.key === "ArrowDown" ? 1 : -1;
    const nextIndex = (currentIndex + offset + buttons.length) % buttons.length;
    buttons[nextIndex]?.focus();
  }

  function renderSaveBar() {
    return (
      <div className="fixed-save-bar" data-testid="fixed-save-bar">
        <span>
          {localApiTokenRequired
            ? t.localApi.tokenRequiredBeforeSave
            : hasChanges
              ? t.settings.unsavedChanges
              : saveMessage}
        </span>
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
          <p>
            {pendingUnsavedAction?.variant === "hide-window"
              ? t.settings.unsavedChangesClosePrompt
              : t.settings.unsavedChangesPrompt}
          </p>
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
              {pendingUnsavedAction?.variant === "hide-window"
                ? t.settings.discardChangesAndClose
                : t.settings.discardChanges}
            </button>
            <button
              type="button"
              disabled={!canSave}
              onClick={() => void saveAndRunPendingAction()}
              data-testid="save-and-continue-unsaved-changes"
            >
              {pendingUnsavedAction?.variant === "hide-window"
                ? t.settings.saveAndCloseWindow
                : t.settings.saveAndContinue}
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
            acceptPersistedConfig(updated, testResult);
          }}
        />
      </section>
    );
  }

  return (
    <section
      className="settings-panel settings-panel--categorized"
      aria-label={t.settings.title}
      data-testid="settings-page"
    >
      <div className="settings-nav-rail">
        <nav
          className="settings-nav"
          role="tablist"
          aria-label={t.settings.categoryNav}
          data-testid="settings-nav"
          onKeyDown={handleSettingsNavKeyDown}
        >
          {SETTINGS_CATEGORY_ORDER.map((category) => (
            <button
              key={category}
              type="button"
              role="tab"
              id={`settings-tab-${category}`}
              className="settings-nav__button"
              data-testid={`settings-nav-${category}`}
              aria-selected={settingsCategory === category}
              aria-controls="settings-category-content"
              tabIndex={settingsCategory === category ? 0 : -1}
              onClick={() => switchSettingsCategory(category)}
            >
              {settingsCategoryLabel(category)}
            </button>
          ))}
        </nav>
        <button
          type="button"
          className="settings-nav__link"
          data-testid="settings-guide-link"
          onClick={() => void openRemoteProviderGuide()}
        >
          {t.settings.openGuide}
        </button>
      </div>
      <div
        className="settings-content"
        role="tabpanel"
        id="settings-category-content"
        aria-labelledby={`settings-tab-${settingsCategory}`}
        ref={settingsContentRef}
      >
      {settingsCategory === "general" ? (
      <section className="settings-section" aria-label={t.settings.general} data-testid="general-settings-section">
        <div className="settings-section-title">
          <h3>{t.settings.general}</h3>
        </div>
        <h4 className="settings-group-title">{t.settings.generalGroupRefresh}</h4>
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
        </div>
        <h4 className="settings-group-title">{t.settings.generalGroupDisplay}</h4>
        <div className="settings-grid general-settings-grid">
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
        <div className="settings-field" data-testid="low-quota-summary">
          <span>{t.settings.lowQuotaWarning}</span>
          <span className="settings-hint">
            {t.settings.lowQuotaSummaryValue(config.lowQuotaWarningThreshold)}{" "}
            {t.settings.lowQuotaAlsoNotifications}
          </span>
          <div className="settings-actions settings-actions--inline">
            <button
              type="button"
              className="button-secondary"
              onClick={() => switchSettingsCategory("notifications")}
            >
              {t.settings.goToNotificationSettings}
            </button>
          </div>
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
        <label>
          {t.settings.theme}
          <select
            data-testid="theme-select"
            value={config.theme ?? "system"}
            onChange={(event) =>
              onChange({
                ...config,
                theme: event.currentTarget.value as NonNullable<AppConfig["theme"]>
              })
            }
          >
            <option value="system">{t.settings.themeSystem}</option>
            <option value="light">{t.settings.themeLight}</option>
            <option value="dark">{t.settings.themeDark}</option>
          </select>
        </label>
        </div>
        <h4 className="settings-group-title">{t.settings.generalGroupStartup}</h4>
        <div className="settings-grid general-settings-grid">
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
      ) : null}
      {settingsCategory === "notifications" ? (
        <NotificationSettingsSection
          settings={config.notifications}
          onChange={(notifications) => onChange({ ...config, notifications })}
          lowQuotaWarningThreshold={config.lowQuotaWarningThreshold}
          onLowQuotaWarningThresholdChange={(lowQuotaWarningThreshold) =>
            onChange({ ...config, lowQuotaWarningThreshold })
          }
          lowQuotaWarningError={lowQuotaWarningError}
        />
      ) : null}
      {settingsCategory === "app-update" ? (
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
                checked={config.appUpdate?.autoCheck ?? true}
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
      ) : null}
      {settingsCategory === "advanced" ? (
        <section
          className="settings-section"
          aria-label={t.settings.categoryAdvanced}
          data-testid="advanced-settings-section"
        >
          <div className="settings-section-title">
            <h3>{t.settings.categoryAdvanced}</h3>
          </div>
          <h4 className="settings-group-title">{t.settings.advancedGroupNetwork}</h4>
          <NetworkProxySettings
            proxy={config.networkProxy}
            onChange={(proxy) => onChange({ ...config, networkProxy: proxy })}
          />
          <LocalApiSettings
            settings={config.localApi}
            refreshKey={`${configStorageInfo?.configPath ?? ""}:${localApiStatusRevision}`}
            onChange={(localApi) => onChange({ ...config, localApi })}
            onTokenRequirementChange={setLocalApiTokenRequired}
          />
          <SecretSecuritySettings />
          <section className="settings-section config-storage-section" aria-label={t.settings.configurationStorage}>
            <div className="settings-section-title">
              <h4 className="settings-group-title">{t.settings.configurationStorage}</h4>
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
                <button
                  type="button"
                  className="path-chip"
                  title={configStorageInfo?.portableMarkerPath}
                  onClick={() => void navigator.clipboard?.writeText(configStorageInfo?.portableMarkerPath ?? "")}
                >
                  {configStorageInfo?.portableMarkerPath ?? t.settings.loading}
                </button>
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
          <h4 className="settings-group-title">{t.settings.advancedGroupDiagnostics}</h4>
          <div className="settings-grid general-settings-grid">
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
          </div>
        </section>
      ) : null}
      {settingsCategory === "providers" ? (
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
      ) : null}
      </div>

      {renderSaveBar()}
      {renderQuotaDataConfirmDialog()}
      {renderUnsavedChangesDialog()}
      {renderRemoveProviderDialog()}
    </section>
  );
}
