import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { AppConfig, AppSnapshot, ProviderConfig, ProviderPreset, ProviderSnapshot } from "../types";

export async function refreshSnapshot(): Promise<AppSnapshot> {
  return invoke<AppSnapshot>("refresh_snapshot");
}

export async function getCachedSnapshot(): Promise<AppSnapshot | null> {
  return invoke<AppSnapshot | null>("get_cached_snapshot");
}

export async function getConfig(): Promise<AppConfig> {
  return invoke<AppConfig>("get_config");
}

export async function getAppVersion(): Promise<string> {
  return invoke<string>("get_app_version");
}

export async function saveConfig(config: AppConfig): Promise<void> {
  return invoke<void>("save_config", { config });
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
