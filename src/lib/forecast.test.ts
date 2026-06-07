import type { AppSnapshot, ProviderSnapshot, QuotaWindow } from "../types";
import {
  alertLevelForRemaining,
  findGlobalLowestWindow,
  findLowestWindowPerProvider,
  formatResetCountdown,
  notificationDedupKey,
  NotificationDeduper,
  suggestionForAlertLevel
} from "./forecast";

function window(id: string, remainingPercent: number, resetAt: string | null = null): QuotaWindow {
  return {
    id,
    label: id,
    used: null,
    limit: null,
    unit: null,
    usedPercent: 100 - remainingPercent,
    remainingPercent,
    resetAt,
    resetText: null,
    confidence: "exact"
  };
}

function provider(id: string, windows: QuotaWindow[]): ProviderSnapshot {
  return {
    id,
    name: id,
    status: "ok",
    source: "mock",
    updatedAt: null,
    windows,
    error: null,
    diagnostics: null,
    metadata: null
  };
}

test("finds_lowest_window_per_provider", () => {
  const forecast = findLowestWindowPerProvider(provider("Codex", [window("5h", 99), window("weekly", 1)]));

  expect(forecast?.window.id).toBe("weekly");
});

test("finds_global_lowest_window", () => {
  const snapshot: AppSnapshot = {
    schemaVersion: 1,
    refreshedAt: "2026-06-08T00:00:00Z",
    providers: [provider("Codex", [window("weekly", 12)]), provider("BigModel", [window("5h", 5)])]
  };

  expect(findGlobalLowestWindow(snapshot)?.providerName).toBe("BigModel");
});

test("maps_remaining_percent_to_alert_level", () => {
  expect(alertLevelForRemaining(20)).toBe("none");
  expect(alertLevelForRemaining(12)).toBe("low");
  expect(alertLevelForRemaining(7)).toBe("critical");
  expect(alertLevelForRemaining(4)).toBe("urgent");
});

test("formats_reset_at_countdown", () => {
  expect(
    formatResetCountdown(
      window("5h", 10, "2026-06-08T02:14:00Z"),
      new Date("2026-06-08T00:00:00Z")
    )
  ).toBe("resets in 2h 14m");
});

test("uses_reset_text_when_reset_at_missing", () => {
  const quotaWindow = { ...window("weekly", 10), resetText: "in 4 hours" };

  expect(formatResetCountdown(quotaWindow)).toBe("in 4 hours");
});

test("generates_suggestion_for_low_critical_urgent", () => {
  expect(suggestionForAlertLevel("low")).toContain("high-value");
  expect(suggestionForAlertLevel("critical")).toContain("Avoid long tasks");
  expect(suggestionForAlertLevel("urgent")).toContain("necessary tasks");
});

test("notification_dedup_keys_include_provider_window_level_reset", () => {
  const forecast = findLowestWindowPerProvider(
    provider("kimi", [window("usage", 4, "2026-06-08T02:14:00Z")])
  );

  expect(notificationDedupKey(forecast!)).toBe("kimi:usage:urgent:2026-06-08T02:14:00Z");
});

test("notification_deduper_allows_one_notification_per_reset_cycle", () => {
  const deduper = new NotificationDeduper();
  const first = findLowestWindowPerProvider(
    provider("kimi", [window("usage", 4, "2026-06-08T02:14:00Z")])
  )!;
  const resetChanged = findLowestWindowPerProvider(
    provider("kimi", [window("usage", 4, "2026-06-09T02:14:00Z")])
  )!;
  const recovered = {
    ...findLowestWindowPerProvider(provider("kimi", [window("usage", 25)]))!,
    alertLevel: "none" as const
  };

  expect(deduper.shouldNotify(first)).toBe(true);
  expect(deduper.shouldNotify(first)).toBe(false);
  expect(deduper.shouldNotify(resetChanged)).toBe(true);
  expect(deduper.shouldNotify(recovered)).toBe(false);
  expect(deduper.shouldNotify(first)).toBe(true);
});
