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

export async function refreshSnapshot(): Promise<AppSnapshot> {
  return invoke<AppSnapshot>("refresh_snapshot");
}

export async function refreshProvider(providerId: string): Promise<AppSnapshot> {
  return invoke<AppSnapshot>("refresh_provider", { providerId });
}

export async function getCachedSnapshot(): Promise<AppSnapshot | null> {
  return invoke<AppSnapshot | null>("get_cached_snapshot");
}

export async function getConfig(): Promise<AppConfig> {
  return invoke<AppConfig>("get_config");
}

export async function getConfigStorageInfo(): Promise<ConfigStorageInfo> {
  return invoke<ConfigStorageInfo>("get_config_storage_info");
}

export async function getAppVersion(): Promise<string> {
  return invoke<string>("get_app_version");
}

export async function saveConfig(config: AppConfig): Promise<void> {
  return invoke<void>("save_config", { config });
}

export async function setPortableMode(enabled: boolean): Promise<ConfigStorageInfo> {
  return invoke<ConfigStorageInfo>("set_portable_mode", { enabled });
}

export async function resetConfig(): Promise<AppConfig> {
  return invoke<AppConfig>("reset_config");
}

export async function openConfigFolder(): Promise<void> {
  return invoke<void>("open_config_folder");
}

export async function getProviderPresets(): Promise<ProviderPreset[]> {
  return invoke<ProviderPreset[]>("get_provider_presets");
}

export async function testProvider(provider: ProviderConfig): Promise<ProviderSnapshot> {
  return invoke<ProviderSnapshot>("test_provider", { provider });
}

export async function exportDiagnostics(outputPath: string): Promise<void> {
  return invoke<void>("export_diagnostics", { outputPath });
}

export async function listenForRefreshRequests(onRefresh: () => void): Promise<() => void> {
  return listen("refresh-requested", onRefresh);
}

export async function listenForSingleInstance(onSecondInstance: (message: string) => void): Promise<() => void> {
  return listen<string>("single-instance", (event) => onSecondInstance(event.payload));
}
