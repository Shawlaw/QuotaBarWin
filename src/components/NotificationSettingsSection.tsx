import { useState } from "react";
import type { NotificationSettings, TestChannelOutcome } from "../types";
import { sendTestNotification } from "../lib/api";
import { useI18n } from "../i18n";
import { QUOTA_EVENT_TYPES, type QuotaEventCategory } from "../lib/quotaEvents";

const DEFAULT_WEBHOOK_TIMEOUT_SECONDS = 10;

const DEFAULT_SETTINGS: NotificationSettings = {
  toastEnabled: false,
  webhookEnabled: false,
  webhookUrl: null,
  webhookTimeoutSeconds: DEFAULT_WEBHOOK_TIMEOUT_SECONDS,
  events: QUOTA_EVENT_TYPES.filter((meta) => meta.type !== "app-started").map((meta) => meta.type),
};

// Presentation order for the event groups; the events themselves always come
// from QUOTA_EVENT_TYPES so this stays in sync with the shared metadata.
const EVENT_CATEGORY_ORDER: QuotaEventCategory[] = ["quota", "provider", "app"];

type NotificationSettingsSectionProps = {
  settings: NotificationSettings | null | undefined;
  onChange: (settings: NotificationSettings) => void;
  lowQuotaWarningThreshold: number;
  onLowQuotaWarningThresholdChange: (value: number) => void;
  lowQuotaWarningError?: string | null;
};

function normalizedSettings(
  settings: NotificationSettings | null | undefined,
): NotificationSettings {
  return settings ?? DEFAULT_SETTINGS;
}

function channelMessage(
  outcome: TestChannelOutcome,
  labels: {
    sent: string;
    disabled: string;
    failed: (detail: string) => string;
  },
): string {
  switch (outcome.status) {
    case "sent":
      return labels.sent;
    case "skipped":
      return labels.disabled;
    default:
      return labels.failed(outcome.detail ?? "");
  }
}

export function NotificationSettingsSection({
  settings: savedSettings,
  onChange,
  lowQuotaWarningThreshold,
  onLowQuotaWarningThresholdChange,
  lowQuotaWarningError = null,
}: NotificationSettingsSectionProps) {
  const { t } = useI18n();
  const settings = normalizedSettings(savedSettings);
  const [webhookUrlText, setWebhookUrlText] = useState(settings.webhookUrl ?? "");
  const [timeoutText, setTimeoutText] = useState(String(settings.webhookTimeoutSeconds));
  const [isTesting, setIsTesting] = useState(false);
  const [testMessage, setTestMessage] = useState<string | null>(null);

  const timeoutValue = Number.parseInt(timeoutText, 10);
  const isTimeoutValid = Number.isInteger(timeoutValue) && timeoutValue >= 1 && timeoutValue <= 60;
  const selectedEvents = new Set(settings.events);

  const commitWebhookUrl = (value: string) => {
    setWebhookUrlText(value);
    onChange({ ...settings, webhookUrl: value.trim() ? value.trim() : null });
  };

  const commitTimeout = (value: string) => {
    setTimeoutText(value);
    const parsed = Number.parseInt(value, 10);
    if (Number.isInteger(parsed) && parsed >= 1 && parsed <= 60) {
      onChange({ ...settings, webhookTimeoutSeconds: parsed });
    }
  };

  const toggleEvent = (eventType: string, checked: boolean) => {
    const next = new Set(selectedEvents);
    if (checked) {
      next.add(eventType);
    } else {
      next.delete(eventType);
    }
    onChange({ ...settings, events: QUOTA_EVENT_TYPES.filter((meta) => next.has(meta.type)).map((meta) => meta.type) });
  };

  const handleTest = async () => {
    setIsTesting(true);
    setTestMessage(null);
    try {
      // Tests the current form draft, so toggling a channel or editing the
      // URL works without saving first.
      const result = await sendTestNotification(settings);
      const lines = [
        channelMessage(result.toast, {
          sent: t.notificationSettings.testToastSent,
          disabled: t.notificationSettings.testToastDisabled,
          failed: t.notificationSettings.testToastFailed,
        }),
        channelMessage(result.webhook, {
          sent: t.notificationSettings.testWebhookSent(result.webhook.statusCode ?? null),
          disabled: t.notificationSettings.testWebhookDisabled,
          failed: t.notificationSettings.testWebhookFailed,
        }),
      ];
      setTestMessage(lines.join("\n"));
    } catch {
      setTestMessage(
        [t.notificationSettings.testToastFailed(""), t.notificationSettings.testWebhookFailed("")].join("\n"),
      );
    } finally {
      setIsTesting(false);
    }
  };

  return (
    <section
      className="settings-section"
      aria-label={t.notificationSettings.title}
      data-testid="notification-settings-section"
    >
      <div className="settings-section-title">
        <h3>{t.notificationSettings.title}</h3>
      </div>
      <h4 className="settings-group-title">{t.notificationSettings.conditionsTitle}</h4>
      <div className="settings-field">
        <span>{t.notificationSettings.eventsLabel}</span>
        {EVENT_CATEGORY_ORDER.map((category) => (
          <div className="notification-event-group" key={category}>
            <span className="notification-event-group__label">
              {t.notificationSettings.eventGroupLabel[category] ?? category}
            </span>
            <div className="notification-event-grid" data-testid={`notification-event-grid-${category}`}>
              {QUOTA_EVENT_TYPES.filter((meta) => meta.category === category).map((meta) => (
                <label key={meta.type} className="checkbox-row">
                  <input
                    type="checkbox"
                    data-testid={`notification-event-${meta.type}`}
                    checked={selectedEvents.has(meta.type)}
                    onChange={(event) => toggleEvent(meta.type, event.currentTarget.checked)}
                  />
                  {t.notificationSettings.eventTypeLabel[meta.type] ?? meta.type}
                </label>
              ))}
            </div>
          </div>
        ))}
        <span className="settings-hint">{t.notificationSettings.eventsHint}</span>
      </div>
      <div className="settings-field">
        <label>
          {t.notificationSettings.lowQuotaThresholdLabel}
          <input
            type="number"
            min={0}
            max={100}
            data-testid="low-quota-warning-input"
            value={lowQuotaWarningThreshold}
            onChange={(event) => onLowQuotaWarningThresholdChange(Number(event.currentTarget.value))}
          />
          {lowQuotaWarningError ? <span className="field-error">{lowQuotaWarningError}</span> : null}
        </label>
        <span className="settings-hint">{t.notificationSettings.lowQuotaThresholdHint}</span>
      </div>
      <h4 className="settings-group-title">{t.notificationSettings.channelsTitle}</h4>
      <div className="settings-field">
        <label className="checkbox-row settings-toggle-row">
          <input
            type="checkbox"
            checked={settings.toastEnabled}
            data-testid="notification-toast-enabled"
            onChange={(event) => onChange({ ...settings, toastEnabled: event.currentTarget.checked })}
          />
          {t.notificationSettings.toastEnabled}
        </label>
        <span className="settings-hint">{t.notificationSettings.toastHint}</span>
      </div>
      <div className="settings-field">
        <label className="checkbox-row settings-toggle-row">
          <input
            type="checkbox"
            checked={settings.webhookEnabled}
            data-testid="notification-webhook-enabled"
            onChange={(event) =>
              onChange({ ...settings, webhookEnabled: event.currentTarget.checked })
            }
          />
          {t.notificationSettings.webhookEnabled}
        </label>
        <span className="settings-hint">{t.notificationSettings.webhookHint}</span>
      </div>
      {settings.webhookEnabled ? (
        <div className="settings-grid notification-webhook-grid">
          <label>
            {t.notificationSettings.webhookUrl}
            <input
              type="text"
              data-testid="notification-webhook-url"
              placeholder={t.notificationSettings.webhookUrlPlaceholder}
              value={webhookUrlText}
              onChange={(event) => commitWebhookUrl(event.currentTarget.value)}
            />
          </label>
          <label>
            {t.notificationSettings.webhookTimeout}
            <input
              type="number"
              min={1}
              max={60}
              data-testid="notification-webhook-timeout"
              value={timeoutText}
              onChange={(event) => commitTimeout(event.currentTarget.value)}
            />
            {!isTimeoutValid ? (
              <span className="field-error" role="alert">
                {t.notificationSettings.webhookTimeoutError}
              </span>
            ) : null}
          </label>
        </div>
      ) : null}
      <div className="settings-actions settings-actions--inline">
        <button
          type="button"
          className="button-secondary"
          data-testid="notification-test-button"
          disabled={isTesting}
          onClick={() => void handleTest()}
        >
          {isTesting ? t.notificationSettings.testing : t.notificationSettings.testButton}
        </button>
      </div>
      <span className="settings-hint">{t.notificationSettings.testButtonHint}</span>
      {testMessage ? (
        <div className="settings-message" data-testid="notification-test-message">
          {testMessage.split("\n").map((line, index) => (
            <div key={index}>{line}</div>
          ))}
        </div>
      ) : null}
    </section>
  );
}
