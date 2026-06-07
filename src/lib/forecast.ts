import type { AppSnapshot, ProviderSnapshot, QuotaWindow } from "../types";

export type AlertLevel = "none" | "low" | "critical" | "urgent";

export type ForecastWindow = {
  providerId: string;
  providerName: string;
  window: QuotaWindow;
  alertLevel: AlertLevel;
  resetLabel: string;
  suggestion: string;
};

export function alertLevelForRemaining(remainingPercent: number | null): AlertLevel {
  if (remainingPercent === null || remainingPercent >= 20) {
    return "none";
  }
  if (remainingPercent >= 10) {
    return "low";
  }
  if (remainingPercent >= 5) {
    return "critical";
  }

  return "urgent";
}

export function findLowestWindowPerProvider(
  provider: ProviderSnapshot,
  now: Date = new Date()
): ForecastWindow | null {
  const window = provider.windows
    .filter((candidate) => candidate.remainingPercent !== null)
    .sort((a, b) => (a.remainingPercent ?? 100) - (b.remainingPercent ?? 100))[0];

  if (!window) {
    return null;
  }

  const alertLevel = alertLevelForRemaining(window.remainingPercent);
  return {
    providerId: provider.id,
    providerName: provider.name,
    window,
    alertLevel,
    resetLabel: formatResetCountdown(window, now),
    suggestion: suggestionForAlertLevel(alertLevel)
  };
}

export function findGlobalLowestWindow(
  snapshot: AppSnapshot | null,
  now: Date = new Date()
): ForecastWindow | null {
  if (!snapshot) {
    return null;
  }

  return snapshot.providers
    .map((provider) => findLowestWindowPerProvider(provider, now))
    .filter((forecast): forecast is ForecastWindow => forecast !== null)
    .sort(
      (a, b) => (a.window.remainingPercent ?? 100) - (b.window.remainingPercent ?? 100)
    )[0] ?? null;
}

export function formatResetCountdown(window: QuotaWindow, now: Date = new Date()): string {
  if (window.resetAt) {
    const resetTime = new Date(window.resetAt).getTime();
    if (Number.isFinite(resetTime)) {
      const diffMinutes = Math.max(0, Math.ceil((resetTime - now.getTime()) / 60000));
      const hours = Math.floor(diffMinutes / 60);
      const minutes = diffMinutes % 60;
      if (hours > 0) {
        return `resets in ${hours}h ${minutes}m`;
      }
      return `resets in ${minutes}m`;
    }
  }

  if (window.resetText) {
    return window.resetText;
  }

  return "reset unknown";
}

export function suggestionForAlertLevel(level: AlertLevel): string {
  switch (level) {
    case "low":
      return "Reserve for high-value tasks";
    case "critical":
      return "Avoid long tasks or large refactors";
    case "urgent":
      return "Use only for necessary tasks; switch provider first";
    case "none":
    default:
      return "Normal use";
  }
}

export function notificationDedupKey(forecast: ForecastWindow): string {
  return [
    forecast.providerId,
    forecast.window.id,
    forecast.alertLevel,
    forecast.window.resetAt ?? forecast.window.resetText ?? "reset-unknown"
  ].join(":");
}

export class NotificationDeduper {
  private readonly sentKeys = new Set<string>();

  shouldNotify(forecast: ForecastWindow): boolean {
    const prefix = `${forecast.providerId}:${forecast.window.id}:`;
    if (forecast.alertLevel === "none") {
      for (const key of [...this.sentKeys]) {
        if (key.startsWith(prefix)) {
          this.sentKeys.delete(key);
        }
      }
      return false;
    }

    const key = notificationDedupKey(forecast);
    if (this.sentKeys.has(key)) {
      return false;
    }

    this.sentKeys.add(key);
    return true;
  }
}
