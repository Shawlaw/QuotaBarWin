import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { AppConfig, AppSnapshot } from "../types";

export async function refreshSnapshot(): Promise<AppSnapshot> {
  return invoke<AppSnapshot>("refresh_snapshot");
}

export async function getCachedSnapshot(): Promise<AppSnapshot | null> {
  return invoke<AppSnapshot | null>("get_cached_snapshot");
}

export async function getConfig(): Promise<AppConfig> {
  return invoke<AppConfig>("get_config");
}

export async function saveConfig(config: AppConfig): Promise<void> {
  return invoke<void>("save_config", { config });
}

export async function listenForRefreshRequests(onRefresh: () => void): Promise<() => void> {
  return listen("refresh-requested", onRefresh);
}
