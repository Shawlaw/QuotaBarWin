import { invoke } from "@tauri-apps/api/core";
import type { AppSnapshot } from "../types";

export async function refreshSnapshot(): Promise<AppSnapshot> {
  return invoke<AppSnapshot>("refresh_snapshot");
}
