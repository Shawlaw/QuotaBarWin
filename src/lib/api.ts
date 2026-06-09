import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type {
  AppConfig,
  AppSnapshot,
  ConfigStorageInfo,
  ProviderConfig,
  ProviderPreset,
  ProviderSnapshot
} from "../types";

function hasTauriInternals(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

const fallbackConfig: AppConfig = {
  schemaVersion: 6,
  refreshIntervalSeconds: 300,
  displayMode: "remaining",
  lowQuotaWarningThreshold: 20,
  launchAtStartup: false,
  logLevel: "info",
  providers: [
    {
      id: "browser-preview",
      name: "Browser Preview",
      enabled: true,
      kind: "mock"
    }
  ]
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
          confidence: "estimated"
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
          confidence: "estimated"
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
          confidence: "estimated"
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
          confidence: "estimated"
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
          confidence: "estimated"
        }
      ]
    }
  ]
};

const fallbackStorageInfo: ConfigStorageInfo = {
  mode: "app-data",
  configPath: "Browser preview: Tauri config path is available in the desktop app",
  configDir: "Browser preview",
  appDataConfigPath: "Browser preview AppData path",
  portableConfigPath: "Browser preview portable path",
  portableMarkerPath: "Browser preview portable marker"
};

export async function refreshSnapshot(): Promise<AppSnapshot> {
  if (!hasTauriInternals()) {
    return fallbackSnapshot;
  }

  return invoke<AppSnapshot>("refresh_snapshot");
}

export async function refreshProvider(providerId: string): Promise<AppSnapshot> {
  if (!hasTauriInternals()) {
    return {
      ...fallbackSnapshot,
      providers: fallbackSnapshot.providers.filter((provider) => provider.id === providerId)
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

export async function getAppVersion(): Promise<string> {
  if (!hasTauriInternals()) {
    return "browser-preview";
  }

  return invoke<string>("get_app_version");
}

export async function saveConfig(config: AppConfig): Promise<void> {
  if (!hasTauriInternals()) {
    void config;
    return;
  }

  return invoke<void>("save_config", { config });
}

export async function setPortableMode(enabled: boolean): Promise<ConfigStorageInfo> {
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

export async function getProviderPresets(): Promise<ProviderPreset[]> {
  if (!hasTauriInternals()) {
    return [];
  }

  return invoke<ProviderPreset[]>("get_provider_presets");
}

export async function testProvider(provider: ProviderConfig): Promise<ProviderSnapshot> {
  if (!hasTauriInternals()) {
    return {
      ...fallbackSnapshot.providers[0],
      id: provider.id,
      name: provider.name
    };
  }

  return invoke<ProviderSnapshot>("test_provider", { provider });
}

export async function exportDiagnostics(outputPath: string): Promise<void> {
  if (!hasTauriInternals()) {
    void outputPath;
    return;
  }

  return invoke<void>("export_diagnostics", { outputPath });
}

export async function listenForRefreshRequests(onRefresh: () => void): Promise<() => void> {
  if (!hasTauriInternals()) {
    void onRefresh;
    return () => undefined;
  }

  return listen("refresh-requested", onRefresh);
}

export async function listenForSingleInstance(onSecondInstance: (message: string) => void): Promise<() => void> {
  if (!hasTauriInternals()) {
    void onSecondInstance;
    return () => undefined;
  }

  return listen<string>("single-instance", (event) => onSecondInstance(event.payload));
}
