import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import type {
  AppConfig,
  AppSnapshot,
  ConfigStorageInfo,
  LocalApiAccessToken,
  LocalApiNetworkInterface,
  LocalApiStatus,
  ProxyConfig,
  ProxyTestResult,
  RemoteProviderCatalogEntry,
  RemoteProviderConfig,
  RemoteProviderManifest,
  ProviderSetupDescriptor,
  ProviderSetupTestResult,
  RegistryMigrationResult,
  SaveProviderSetupRequest,
} from "../types";
import { DEFAULT_REMOTE_PROVIDER_REGISTRY_URL } from "./defaults";

function hasTauriInternals(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

const fallbackConfig: AppConfig = {
  schemaVersion: 20,
  refreshIntervalSeconds: 300,
  displayMode: "remaining",
  lowQuotaWarningThreshold: 20,
  launchAtStartup: false,
  logLevel: "info",
  logMaxBytes: 10 * 1024 * 1024,
  logQuotaData: false,
  language: "zh-CN",
  networkProxy: null,
  appUpdate: {
    autoCheck: true,
  },
  localApi: {
    enabled: false,
    bindTarget: { kind: "loopback" },
    port: 41833,
  },
  remoteProviderRegistry: {
    registryUrl: DEFAULT_REMOTE_PROVIDER_REGISTRY_URL,
    providerProxyUrl: null,
    autoUpdate: true,
  },
  providers: [],
};

const fallbackSnapshot: AppSnapshot = {
  schemaVersion: 1,
  refreshedAt: new Date().toISOString(),
  providers: [
    {
      id: "browser-preview",
      name: "Browser Preview",
      status: "ok",
      source: "mock",
      updatedAt: new Date().toISOString(),
      error: null,
      diagnostics: null,
      metadata: null,
      windows: [
        {
          id: "5h",
          label: "5h window",
          used: 28,
          limit: 100,
          unit: "percent",
          usedPercent: 28,
          remainingPercent: 72,
          resetAt: null,
          resetText: "resets in 2h 30m",
          confidence: "estimated",
        },
        {
          id: "weekly",
          label: "Weekly limit",
          used: 12,
          limit: 100,
          unit: "percent",
          usedPercent: 12,
          remainingPercent: 88,
          resetAt: null,
          resetText: "resets Fri 10:01",
          confidence: "estimated",
        },
        {
          id: "daily",
          label: "Daily",
          used: 55,
          limit: 100,
          unit: "percent",
          usedPercent: 55,
          remainingPercent: 45,
          resetAt: null,
          resetText: "resets tomorrow",
          confidence: "estimated",
        },
        {
          id: "monthly",
          label: "Monthly",
          used: 70,
          limit: 100,
          unit: "percent",
          usedPercent: 70,
          remainingPercent: 30,
          resetAt: null,
          resetText: "resets 2026/06/30",
          confidence: "estimated",
        },
        {
          id: "token",
          label: "Token pool",
          used: 92,
          limit: 100,
          unit: "percent",
          usedPercent: 92,
          remainingPercent: 8,
          resetAt: null,
          resetText: "resets soon",
          confidence: "estimated",
        },
      ],
    },
  ],
};

const fallbackStorageInfo: ConfigStorageInfo = {
  mode: "app-data",
  configPath:
    "Browser preview: Tauri config path is available in the desktop app",
  configDir: "Browser preview",
  appDataConfigPath: "Browser preview AppData path",
  portableConfigPath: "Browser preview portable path",
  portableMarkerPath: "Browser preview portable marker",
};

export async function refreshSnapshot(): Promise<AppSnapshot> {
  if (!hasTauriInternals()) {
    return fallbackSnapshot;
  }

  return invoke<AppSnapshot>("refresh_snapshot");
}

export async function refreshProvider(
  providerId: string,
): Promise<AppSnapshot> {
  if (!hasTauriInternals()) {
    return {
      ...fallbackSnapshot,
      providers: fallbackSnapshot.providers.filter(
        (provider) => provider.id === providerId,
      ),
    };
  }

  return invoke<AppSnapshot>("refresh_provider", { providerId });
}

export async function getCachedSnapshot(): Promise<AppSnapshot | null> {
  if (!hasTauriInternals()) {
    return fallbackSnapshot;
  }

  return invoke<AppSnapshot | null>("get_cached_snapshot");
}

export async function getConfig(): Promise<AppConfig> {
  if (!hasTauriInternals()) {
    return fallbackConfig;
  }

  return invoke<AppConfig>("get_config");
}

export async function getConfigStorageInfo(): Promise<ConfigStorageInfo> {
  if (!hasTauriInternals()) {
    return fallbackStorageInfo;
  }

  return invoke<ConfigStorageInfo>("get_config_storage_info");
}

export async function getLocalApiStatus(): Promise<LocalApiStatus> {
  if (!hasTauriInternals()) {
    return {
      enabled: false,
      running: false,
      endpoints: [],
      requiresAuth: false,
      tokenConfigured: false,
      error: null,
    };
  }

  return invoke<LocalApiStatus>("get_local_api_status");
}

export async function listLocalApiNetworkInterfaces(): Promise<LocalApiNetworkInterface[]> {
  if (!hasTauriInternals()) {
    return [];
  }

  return invoke<LocalApiNetworkInterface[]>("list_local_api_network_interfaces");
}

export async function getLocalApiAccessToken(): Promise<LocalApiAccessToken> {
  if (!hasTauriInternals()) {
    throw new Error("Local integration API tokens are available only in the desktop app");
  }

  return invoke<LocalApiAccessToken>("get_local_api_access_token");
}

export async function setLocalApiAccessToken(
  token?: string,
): Promise<LocalApiAccessToken> {
  if (!hasTauriInternals()) {
    throw new Error("Local integration API tokens are available only in the desktop app");
  }

  return invoke<LocalApiAccessToken>("set_local_api_access_token", { token });
}

export async function getAppVersion(): Promise<string> {
  if (!hasTauriInternals()) {
    return "browser-preview";
  }

  return invoke<string>("get_app_version");
}

export async function openProjectGithub(): Promise<void> {
  if (!hasTauriInternals()) {
    window.open("https://github.com/Shawlaw/QuotaBarWin", "_blank", "noopener,noreferrer");
    return;
  }

  return invoke<void>("open_project_github");
}

export async function saveConfig(config: AppConfig): Promise<void> {
  if (!hasTauriInternals()) {
    void config;
    return;
  }

  return invoke<void>("save_config", { config });
}

export async function setPortableMode(
  enabled: boolean,
): Promise<ConfigStorageInfo> {
  if (!hasTauriInternals()) {
    void enabled;
    return fallbackStorageInfo;
  }

  return invoke<ConfigStorageInfo>("set_portable_mode", { enabled });
}

export async function resetConfig(): Promise<AppConfig> {
  if (!hasTauriInternals()) {
    return fallbackConfig;
  }

  return invoke<AppConfig>("reset_config");
}

export async function openConfigFolder(): Promise<void> {
  if (!hasTauriInternals()) {
    return;
  }

  return invoke<void>("open_config_folder");
}

export async function openRemoteProviderGuide(): Promise<void> {
  if (!hasTauriInternals()) {
    window.open("/remote-provider-guide.html", "_blank", "noopener,noreferrer");
    return;
  }

  return invoke<void>("open_remote_provider_guide");
}

export type UpdateInfo = {
  id: string;
  available: boolean;
  newChecksum: string | null;
  updateManifestUrl?: string | null;
  currentVersion?: string | null;
  newVersion?: string | null;
  checkedAt?: string | null;
};

export type AppUpdateInfo = {
  configured: boolean;
  currentVersion: string;
  available: boolean;
  version: string | null;
  notesUrl: string | null;
  downloaded: boolean;
  checkedAt?: string | null;
  error?: string | null;
  dismissed?: boolean;
};

export type AppUpdateStatusEvent = {
  info: AppUpdateInfo;
  animate: boolean;
};

const unavailableAppUpdate: AppUpdateInfo = {
  configured: false,
  currentVersion: "Browser Preview",
  available: false,
  version: null,
  notesUrl: null,
  downloaded: false,
  checkedAt: null,
  error: null,
  dismissed: false,
};

export async function getAppUpdateStatus(): Promise<AppUpdateInfo> {
  if (!hasTauriInternals()) {
    return unavailableAppUpdate;
  }

  return invoke<AppUpdateInfo>("get_app_update_status");
}

export async function checkAppUpdate(): Promise<AppUpdateInfo> {
  if (!hasTauriInternals()) {
    return unavailableAppUpdate;
  }

  return invoke<AppUpdateInfo>("check_app_update");
}

export async function dismissAppUpdateNotice(): Promise<AppUpdateInfo> {
  if (!hasTauriInternals()) {
    return { ...unavailableAppUpdate, dismissed: true };
  }

  return invoke<AppUpdateInfo>("dismiss_app_update_notice");
}

export async function downloadAppUpdate(): Promise<AppUpdateInfo> {
  if (!hasTauriInternals()) {
    return unavailableAppUpdate;
  }

  return invoke<AppUpdateInfo>("download_app_update");
}

export async function applyAppUpdate(): Promise<void> {
  if (!hasTauriInternals()) {
    return;
  }

  return invoke<void>("apply_app_update");
}

export async function openAppUpdateNotes(notesUrl: string): Promise<void> {
  if (!hasTauriInternals()) {
    window.open(notesUrl, "_blank", "noopener,noreferrer");
    return;
  }

  return invoke<void>("open_app_update_notes", { notesUrl });
}

export async function getNetworkProxy(): Promise<ProxyConfig | null> {
  if (!hasTauriInternals()) {
    return null;
  }

  return invoke<ProxyConfig | null>("get_network_proxy");
}

export async function setNetworkProxy(
  proxy: ProxyConfig | null,
): Promise<void> {
  if (!hasTauriInternals()) {
    void proxy;
    return;
  }

  return invoke<void>("set_network_proxy", { proxy });
}

export async function testNetworkProxy(
  proxy: ProxyConfig,
  targetUrl: string,
): Promise<ProxyTestResult> {
  if (!hasTauriInternals()) {
    throw new Error("Proxy testing is not available in browser preview");
  }

  return invoke<ProxyTestResult>("test_network_proxy", { proxy, targetUrl });
}

export type RegistryInstallFailure = {
  id: string;
  error: string;
};

export type RegistryInstallResult = {
  installed: RemoteProviderConfig[];
  skipped: string[];
  failed: RegistryInstallFailure[];
};

export async function previewRemoteProviderRegistry(
  url: string,
  proxyUrl: string | null,
): Promise<RemoteProviderCatalogEntry[]> {
  if (!hasTauriInternals()) {
    void url;
    void proxyUrl;
    return [];
  }

  return invoke<RemoteProviderCatalogEntry[]>("preview_remote_provider_registry", {
    url,
    proxyUrl,
  });
}

export async function installRemoteProviderManifest(
  url: string,
  checksum: string | null,
  proxyUrl: string | null,
  autoUpdate: boolean,
): Promise<RemoteProviderConfig> {
  if (!hasTauriInternals()) {
    void url;
    void checksum;
    void proxyUrl;
    void autoUpdate;
    throw new Error(
      "Installing a remote provider manifest is not available in browser preview",
    );
  }

  return invoke<RemoteProviderConfig>("install_remote_provider_manifest", {
    url,
    checksum,
    proxyUrl,
    autoUpdate,
  });
}

export async function installRemoteProviderRegistry(
  url: string,
  proxyUrl: string | null,
  autoUpdate: boolean,
): Promise<RegistryInstallResult> {
  if (!hasTauriInternals()) {
    void url;
    void proxyUrl;
    void autoUpdate;
    throw new Error(
      "Installing remote provider registry is not available in browser preview",
    );
  }

  return invoke<RegistryInstallResult>("install_remote_provider_registry", {
    url,
    proxyUrl,
    autoUpdate,
  });
}

export async function migrateRemoteProvidersToRegistry(
  url: string,
  proxyUrl: string | null,
): Promise<RegistryMigrationResult> {
  if (!hasTauriInternals()) {
    void url;
    void proxyUrl;
    throw new Error(
      "Migrating installed remote providers is not available in browser preview",
    );
  }

  return invoke<RegistryMigrationResult>("migrate_remote_providers_to_registry", {
    url,
    proxyUrl,
  });
}

export async function getInstalledRemoteProviderManifest(
  id: string,
): Promise<RemoteProviderManifest> {
  if (!hasTauriInternals()) {
    void id;
    return {
      schemaVersion: 1,
      id: "browser-preview",
      displayName: "Browser Preview",
      runtime: "node",
      entry: "provider.cjs",
      requiredEnvVars: [],
      output: "provider-snapshot-v1",
      permissions: [],
      defaultConfig: {},
      parameters: [],
      checksums: {},
    };
  }

  return invoke<RemoteProviderManifest>("get_installed_remote_provider_manifest", { id });
}

export async function getProviderSetup(
  providerId: string,
): Promise<ProviderSetupDescriptor> {
  if (!hasTauriInternals()) {
    return {
      providerId,
      providerType: "browser-preview",
      displayName: "Browser Preview",
      setupState: "ready",
      fields: [],
      hasUnknownEnvVars: false,
      canAutoDetect: true,
    };
  }

  return invoke<ProviderSetupDescriptor>("get_provider_setup", { providerId });
}

export async function saveProviderSetup(
  request: SaveProviderSetupRequest,
): Promise<ProviderSetupDescriptor> {
  if (!hasTauriInternals()) {
    void request;
    throw new Error("Saving Provider setup is not available in browser preview");
  }

  return invoke<ProviderSetupDescriptor>("save_provider_setup", { request });
}

export async function testProviderSetup(
  providerId: string,
): Promise<ProviderSetupTestResult> {
  if (!hasTauriInternals()) {
    return {
      success: true,
      provider: null,
    };
  }

  return invoke<ProviderSetupTestResult>("test_provider_setup", { providerId });
}

export async function removeRemoteProvider(
  id: string,
  deleteManagedSecrets = true,
): Promise<void> {
  if (!hasTauriInternals()) {
    void id;
    void deleteManagedSecrets;
    return;
  }

  return invoke<void>("remove_remote_provider", { id, deleteManagedSecrets });
}

export async function refreshRemoteProvider(id: string): Promise<UpdateInfo> {
  if (!hasTauriInternals()) {
    void id;
    return {
      id: "",
      available: false,
      newChecksum: null,
      updateManifestUrl: null,
      currentVersion: null,
      newVersion: null,
      checkedAt: null
    };
  }

  return invoke<UpdateInfo>("refresh_remote_provider", { id });
}

export async function checkRemoteUpdates(): Promise<UpdateInfo[]> {
  if (!hasTauriInternals()) {
    return [];
  }

  return invoke<UpdateInfo[]>("check_remote_updates");
}

export async function applyRemoteUpdate(
  id: string,
  updateManifestUrl?: string | null
): Promise<void> {
  if (!hasTauriInternals()) {
    void id;
    void updateManifestUrl;
    return;
  }

  return invoke<void>("apply_remote_update", {
    id,
    updateManifestUrl: updateManifestUrl ?? null
  });
}

export async function exportDiagnostics(outputPath: string): Promise<void> {
  if (!hasTauriInternals()) {
    void outputPath;
    return;
  }

  return invoke<void>("export_diagnostics", { outputPath });
}

export async function listenForRefreshRequests(
  onRefresh: () => void,
): Promise<() => void> {
  if (!hasTauriInternals()) {
    void onRefresh;
    return () => undefined;
  }

  return listen("refresh-requested", onRefresh);
}

export async function listenForSnapshotUpdates(
  onSnapshot: (snapshot: AppSnapshot) => void,
): Promise<() => void> {
  if (!hasTauriInternals()) {
    void onSnapshot;
    return () => undefined;
  }

  return listen<AppSnapshot>("snapshot-refreshed", (event) =>
    onSnapshot(event.payload),
  );
}

export async function listenForAppUpdateStatus(
  onStatus: (status: AppUpdateStatusEvent) => void,
): Promise<() => void> {
  if (!hasTauriInternals()) {
    void onStatus;
    return () => undefined;
  }

  return listen<AppUpdateStatusEvent>("app-update-status-changed", (event) =>
    onStatus(event.payload),
  );
}

export async function listenForTrayPopupShown(
  onShown: (presentationId: number) => void,
): Promise<() => void> {
  if (!hasTauriInternals()) {
    void onShown;
    return () => undefined;
  }

  return listen<{ presentationId: number }>("tray-popup-shown", (event) =>
    onShown(event.payload.presentationId),
  );
}

export async function getTrayPopupPresentationId(): Promise<number> {
  if (!hasTauriInternals()) {
    return 0;
  }

  return invoke<number>("get_tray_popup_presentation_id");
}

export async function listenForSingleInstance(
  onSecondInstance: (message: string) => void,
): Promise<() => void> {
  if (!hasTauriInternals()) {
    void onSecondInstance;
    return () => undefined;
  }

  return listen<string>("single-instance", (event) =>
    onSecondInstance(event.payload),
  );
}

export async function hideCurrentWindow(): Promise<void> {
  if (!hasTauriInternals()) {
    return;
  }

  await getCurrentWindow().hide();
}

export async function hideTrayPopup(): Promise<void> {
  if (!hasTauriInternals()) {
    return;
  }

  return invoke<void>("hide_tray_popup");
}

export async function showMainWindow(): Promise<void> {
  if (!hasTauriInternals()) {
    return;
  }

  return invoke<void>("show_main_window");
}

export async function showApplicationUpdate(): Promise<void> {
  if (!hasTauriInternals()) {
    return;
  }

  return invoke<void>("show_application_update");
}

export async function getApplicationUpdateNavigationRequest(): Promise<number> {
  if (!hasTauriInternals()) {
    return 0;
  }

  return invoke<number>("get_app_update_navigation_request");
}

export async function listenForApplicationUpdateRequests(
  onOpen: (requestId: number) => void,
): Promise<() => void> {
  if (!hasTauriInternals()) {
    void onOpen;
    return () => undefined;
  }

  return listen<number>("open-app-update", (event) => onOpen(event.payload));
}

export async function resetTrayPopupSize(): Promise<void> {
  if (!hasTauriInternals()) {
    return;
  }

  return invoke<void>("reset_tray_popup_size");
}

export async function setTrayPopupAutoHeight(height: number): Promise<void> {
  if (!hasTauriInternals()) {
    return;
  }

  return invoke<void>("set_tray_popup_auto_height", { height });
}

export async function startDraggingCurrentWindow(): Promise<void> {
  if (!hasTauriInternals()) {
    return;
  }

  return invoke<void>("start_tray_popup_dragging");
}

export async function startResizingCurrentWindow(): Promise<void> {
  if (!hasTauriInternals()) {
    return;
  }

  return invoke<void>("start_tray_popup_resizing");
}
