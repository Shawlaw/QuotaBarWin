import type { ProviderSnapshot, QuotaWindow } from "../types";
import { en, type I18nCatalog } from "../i18n/catalog";

export type DisplayMode = "remaining" | "used";

type RawProvider = Partial<ProviderSnapshot> & {
  quotaWindows?: QuotaWindow[];
  fiveHourRemaining?: number;
  weeklyRemaining?: number;
  errorMessage?: string;
};

export function normalizeProviderStatus(raw: unknown): ProviderSnapshot {
  const provider = (raw && typeof raw === "object" ? raw : {}) as RawProvider;
  const windows = Array.isArray(provider.windows)
    ? provider.windows
    : Array.isArray(provider.quotaWindows)
      ? provider.quotaWindows
      : legacyWindows(provider);

  return {
    id: provider.id ?? "unknown-provider",
    name: provider.name ?? "Unknown provider",
    status: provider.status ?? (provider.error || provider.errorMessage ? "error" : "unknown"),
    source: provider.source ?? "mock",
    updatedAt: provider.updatedAt ?? null,
    windows,
    error: provider.error ?? provider.errorMessage ?? null,
    diagnostics: provider.diagnostics ?? null,
    metadata: provider.metadata ?? null
  };
}

export function calculateProviderStatus(
  provider: ProviderSnapshot,
  lowQuotaWarningThreshold = 20
): ProviderSnapshot["status"] {
  if (provider.status === "stale") {
    return "stale";
  }

  if (provider.status === "error" || provider.error) {
    return "error";
  }

  if (provider.windows.some((window) => windowStatus(window, lowQuotaWarningThreshold) === "warning")) {
    return "warning";
  }

  if (provider.status === "warning") {
    return "warning";
  }

  if (provider.windows.length > 0) {
    return "ok";
  }

  return "unknown";
}

export function windowStatus(
  window: QuotaWindow,
  lowQuotaWarningThreshold = 20
): "ok" | "warning" | "unknown" {
  if (window.remainingPercent === null || window.remainingPercent === undefined) {
    return "unknown";
  }

  return window.remainingPercent <= lowQuotaWarningThreshold ? "warning" : "ok";
}

export function displayPercentForWindow(window: QuotaWindow, displayMode: DisplayMode): number | null {
  if (displayMode === "remaining") {
    return window.remainingPercent ?? null;
  }

  if (window.usedPercent !== null && window.usedPercent !== undefined) {
    return window.usedPercent;
  }

  if (window.remainingPercent !== null && window.remainingPercent !== undefined) {
    return 100 - window.remainingPercent;
  }

  return null;
}

export function formatDisplayValue(
  window: QuotaWindow,
  displayMode: DisplayMode,
  catalog: I18nCatalog = en
): string {
  const percent = displayPercentForWindow(window, displayMode);
  if (percent === null) {
    return displayMode === "remaining" ? catalog.format.remainingUnknown : catalog.format.usageUnknown;
  }

  return catalog.format.displayValue(Math.round(Math.min(100, Math.max(0, percent))), displayMode);
}

export function formatShortDateTime(value: string | null | undefined, now = new Date()): string | null {
  if (!value) {
    return null;
  }

  const date = new Date(value);
  if (!Number.isFinite(date.getTime())) {
    return null;
  }

  const sameDay = date.toDateString() === now.toDateString();
  const withinWeek =
    date.getTime() > now.getTime() && date.getTime() - now.getTime() < 7 * 24 * 60 * 60 * 1000;

  if (sameDay) {
    return date.toLocaleTimeString(undefined, { hour: "2-digit", minute: "2-digit" });
  }

  if (withinWeek) {
    return date.toLocaleString(undefined, {
      weekday: "short",
      hour: "2-digit",
      minute: "2-digit"
    });
  }

  return date.toLocaleString(undefined, {
    year: "numeric",
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit"
  });
}

export function formatQuotaReset(
  window: QuotaWindow,
  now = new Date(),
  catalog: I18nCatalog = en
): string | null {
  const resetAt = formatShortDateTime(window.resetAt, now);
  if (resetAt) {
    return catalog.format.resets(resetAt);
  }

  return window.resetText ?? null;
}

function legacyWindows(provider: RawProvider): QuotaWindow[] {
  const windows: QuotaWindow[] = [];
  if (typeof provider.fiveHourRemaining === "number") {
    windows.push(legacyWindow("5h", "5h", provider.fiveHourRemaining));
  }
  if (typeof provider.weeklyRemaining === "number") {
    windows.push(legacyWindow("weekly", "Weekly limit", provider.weeklyRemaining));
  }

  return windows;
}

function legacyWindow(id: string, label: string, remainingPercent: number): QuotaWindow {
  return {
    id,
    label,
    used: null,
    limit: null,
    unit: "percent",
    usedPercent: 100 - remainingPercent,
    remainingPercent,
    resetAt: null,
    resetText: null,
    confidence: "unknown"
  };
}
