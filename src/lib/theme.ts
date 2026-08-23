import type { AppTheme } from "../types";

export const DEFAULT_APP_THEME: AppTheme = "system";
export type ResolvedAppTheme = Exclude<AppTheme, "system">;

export function resolveAppTheme(theme: AppTheme | undefined): ResolvedAppTheme {
  if (theme === "light" || theme === "dark") {
    return theme;
  }

  return window.matchMedia?.("(prefers-color-scheme: dark)").matches ? "dark" : "light";
}

export function applyAppTheme(theme: AppTheme | undefined): void {
  if (typeof document === "undefined") {
    return;
  }

  document.documentElement.dataset.theme = resolveAppTheme(theme ?? DEFAULT_APP_THEME);
}
