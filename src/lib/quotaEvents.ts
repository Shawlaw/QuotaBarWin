import type { I18nCatalog } from "../i18n";
import type { QuotaEvent } from "../types";

export type QuotaEventCategory = "quota" | "provider" | "app";

export type QuotaEventTypeMeta = {
  type: string;
  category: QuotaEventCategory;
};

// Keep in sync with the notification event constants in src-tauri/src/config.rs.
export const QUOTA_EVENT_TYPES: QuotaEventTypeMeta[] = [
  { type: "quota-reset", category: "quota" },
  { type: "quota-recovered-unexpected", category: "quota" },
  { type: "quota-exhausted", category: "quota" },
  { type: "quota-low", category: "quota" },
  { type: "provider-error", category: "provider" },
  { type: "provider-recovered", category: "provider" },
  { type: "app-update-applied", category: "app" },
  { type: "app-started", category: "app" },
];

export function eventCategory(eventType: string): QuotaEventCategory {
  return QUOTA_EVENT_TYPES.find((meta) => meta.type === eventType)?.category ?? "app";
}

export function formatEventPercent(value: number): string {
  return Number.isInteger(value) ? String(value) : value.toFixed(1);
}

function detailNumber(details: Record<string, unknown> | null | undefined, key: string): number | null {
  const value = details?.[key];
  return typeof value === "number" && Number.isFinite(value) ? value : null;
}

function detailBoolean(details: Record<string, unknown> | null | undefined, key: string): boolean {
  return details?.[key] === true;
}

function detailString(details: Record<string, unknown> | null | undefined, key: string): string | null {
  const value = details?.[key];
  return typeof value === "string" && value.length > 0 ? value : null;
}

export function eventMessage(t: I18nCatalog, event: QuotaEvent): string {
  const details = event.details ?? null;
  const provider = event.providerName ?? "";
  const window = event.windowLabel ?? "";
  const subject = window ? `${provider} ${window}` : provider;

  switch (event.eventType) {
    case "app-started":
      return t.events.messages.appStarted(
        detailBoolean(details, "startedHidden"),
        detailString(details, "version"),
      );
    case "app-update-applied":
      return t.events.messages.appUpdateApplied(detailString(details, "version") ?? "-");
    case "quota-reset": {
      const remaining = detailNumber(details, "remainingPercent");
      return t.events.messages.quotaReset(
        subject,
        remaining === null ? null : formatEventPercent(remaining),
      );
    }
    case "quota-recovered-unexpected": {
      const before = detailNumber(details, "usedPercentBefore");
      const after = detailNumber(details, "usedPercentAfter");
      return t.events.messages.quotaRecoveredUnexpected(
        subject,
        before === null ? "?" : formatEventPercent(before),
        after === null ? "?" : formatEventPercent(after),
      );
    }
    case "quota-exhausted":
      return t.events.messages.quotaExhausted(subject);
    case "quota-low": {
      const remaining = detailNumber(details, "remainingPercent");
      return t.events.messages.quotaLow(subject, remaining === null ? "?" : formatEventPercent(remaining));
    }
    case "provider-error":
      return t.events.messages.providerError(provider, detailString(details, "error"));
    case "provider-recovered":
      return t.events.messages.providerRecovered(provider);
    default:
      return t.events.messages.unknown(event.eventType);
  }
}

export function formatEventTime(occurredAt: string): string {
  const date = new Date(occurredAt);
  if (Number.isNaN(date.getTime())) {
    return occurredAt;
  }
  return date.toLocaleString();
}
