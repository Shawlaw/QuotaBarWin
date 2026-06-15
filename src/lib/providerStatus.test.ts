import { describe, expect, test, vi } from "vitest";
import type { ProviderSnapshot } from "../types";
import {
  calculateProviderStatus,
  displayPercentForWindow,
  formatDisplayValue,
  formatQuotaReset,
  normalizeProviderStatus
} from "./providerStatus";

const provider: ProviderSnapshot = {
  id: "provider",
  name: "Provider",
  status: "ok",
  source: "remote",
  updatedAt: null,
  error: null,
  diagnostics: null,
  metadata: null,
  windows: [
    {
      id: "daily",
      label: "Daily",
      used: null,
      limit: null,
      unit: "percent",
      usedPercent: 30,
      remainingPercent: 70,
      resetAt: null,
      resetText: null,
      confidence: "exact"
    }
  ]
};

describe("normalizeProviderStatus", () => {
  test("converts_legacy_5h_and_weekly_fields_to_windows", () => {
    const normalized = normalizeProviderStatus({
      id: "legacy",
      name: "Legacy",
      fiveHourRemaining: 90,
      weeklyRemaining: 80
    });

    expect(normalized.windows).toHaveLength(2);
    expect(normalized.windows.map((window) => window.label)).toEqual(["5h", "Weekly limit"]);
  });

  test("keeps_existing_quota_windows", () => {
    const normalized = normalizeProviderStatus({ ...provider, quotaWindows: provider.windows, windows: undefined });

    expect(normalized.windows).toEqual(provider.windows);
  });

  test("returns_empty_windows_for_missing_data", () => {
    const normalized = normalizeProviderStatus({ id: "empty", name: "Empty" });

    expect(normalized.windows).toEqual([]);
    expect(normalized.status).toBe("unknown");
  });
});

describe("calculateProviderStatus", () => {
  test("marks_ok_when_windows_are_above_threshold", () => {
    expect(calculateProviderStatus(provider, 20)).toBe("ok");
  });

  test("marks_warning_when_any_window_is_below_threshold", () => {
    expect(
      calculateProviderStatus(
        {
          ...provider,
          windows: [{ ...provider.windows[0], remainingPercent: 12 }]
        },
        20
      )
    ).toBe("warning");
  });

  test("marks_warning_when_balance_is_below_absolute_warning_amount", () => {
    expect(
      calculateProviderStatus(
        {
          ...provider,
          windows: [
            {
              ...provider.windows[0],
              remaining: 12,
              used: 88,
              limit: 100,
              unit: "CNY",
              remainingPercent: 88,
              warningRemaining: 20
            }
          ]
        },
        20
      )
    ).toBe("warning");
  });

  test('marks_stale_when_provider_status_is_stale', () => {
    expect(calculateProviderStatus({ ...provider, status: 'stale' }, 20)).toBe('stale');
  });

  test("marks_error_when_provider_refresh_failed", () => {
    expect(calculateProviderStatus({ ...provider, status: "error", error: "failed" }, 20)).toBe("error");
  });
});

describe("display mode", () => {
  test("uses_remaining_percent_in_remaining_mode", () => {
    expect(formatDisplayValue(provider.windows[0], "remaining")).toBe("70% remaining");
  });

  test("uses_or_derives_used_percent_in_used_mode", () => {
    expect(displayPercentForWindow({ ...provider.windows[0], usedPercent: null }, "used")).toBe(30);
    expect(formatDisplayValue(provider.windows[0], "used")).toBe("30% used");
  });

  test("handles_missing_percent_without_fake_progress", () => {
    expect(formatDisplayValue({ ...provider.windows[0], remainingPercent: null, usedPercent: null }, "remaining")).toBe(
      "Remaining unknown"
    );
  });
});

describe("formatQuotaReset", () => {
  test("formats_same_day_and_week_reset_times", () => {
    vi.spyOn(Date.prototype, "toLocaleTimeString").mockReturnValue("11:35");
    vi.spyOn(Date.prototype, "toLocaleString").mockReturnValue("Fri 10:01");
    const now = new Date("2026-06-09T10:00:00+08:00");

    expect(formatQuotaReset({ ...provider.windows[0], resetAt: "2026-06-09T03:35:00Z" }, now)).toBe("resets 11:35");
    expect(formatQuotaReset({ ...provider.windows[0], resetAt: "2026-06-12T02:01:00Z" }, now)).toBe(
      "resets Fri 10:01"
    );
  });

  test("falls_back_to_reset_text_for_invalid_dates", () => {
    expect(formatQuotaReset({ ...provider.windows[0], resetAt: "invalid", resetText: "resets soon" })).toBe(
      "resets soon"
    );
  });
});
