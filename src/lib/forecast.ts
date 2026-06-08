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
  const resetDate = resetDateForWindow(window, now);
  if (resetDate) {
    return `resets at ${formatLocalResetTime(resetDate)}`;
  }

  if (window.resetText) {
    return window.resetText;
  }

  return "reset unknown";
}

export function formatLocalResetTime(date: Date): string {
  return date.toLocaleString(undefined, {
    year: "numeric",
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
    timeZoneName: "short"
  });
}

export function resetDateForWindow(window: QuotaWindow, now: Date = new Date()): Date | null {
  if (window.resetAt) {
    const resetDate = new Date(window.resetAt);
    if (Number.isFinite(resetDate.getTime())) {
      return resetDate;
    }
  }

  if (window.resetText) {
    return parseRelativeResetText(window.resetText, now);
  }

  return null;
}

export function parseRelativeResetText(text: string, now: Date = new Date()): Date | null {
  const normalized = text.trim().toLowerCase();
  const pattern =
    /(\d+(?:\.\d+)?)\s*(weeks?|w|days?|d|hours?|hrs?|h|minutes?|mins?|m|seconds?|secs?|s)\b/g;
  let totalMs = 0;
  let matched = false;

  for (const match of normalized.matchAll(pattern)) {
    const value = Number(match[1]);
    if (!Number.isFinite(value)) {
      continue;
    }

    matched = true;
    const unit = match[2];
    if (unit === "w" || unit.startsWith("week")) {
      totalMs += value * 7 * 24 * 60 * 60 * 1000;
    } else if (unit === "d" || unit.startsWith("day")) {
      totalMs += value * 24 * 60 * 60 * 1000;
    } else if (unit === "h" || unit.startsWith("hr") || unit.startsWith("hour")) {
      totalMs += value * 60 * 60 * 1000;
    } else if (unit === "m" || unit.startsWith("min")) {
      totalMs += value * 60 * 1000;
    } else if (unit === "s" || unit.startsWith("sec")) {
      totalMs += value * 1000;
    }
  }

  if (!matched) {
    return null;
  }

  return new Date(now.getTime() + totalMs);
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
