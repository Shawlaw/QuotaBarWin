import { useState } from "react";
import type { NotificationSettings, WebhookEndpoint } from "../types";
import { sendTestNotification, openWebhookTemplateGuide } from "../lib/api";
import { useI18n } from "../i18n";
import { QUOTA_EVENT_TYPES, type QuotaEventCategory } from "../lib/quotaEvents";

const DEFAULT_WEBHOOK_TIMEOUT_SECONDS = 10;

// Event types recorded in the history but excluded from notification
// defaults, mirroring DEFAULT_NOTIFICATION_EVENTS in src-tauri/src/config.rs.
const HISTORY_ONLY_EVENT_TYPES = new Set(["app-started", "quota-reset-time-changed"]);

const DEFAULT_SETTINGS: NotificationSettings = {
  toastEnabled: false,
  webhookEnabled: false,
  webhooks: [],
  events: QUOTA_EVENT_TYPES.filter((meta) => !HISTORY_ONLY_EVENT_TYPES.has(meta.type)).map(
    (meta) => meta.type,
  ),
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

function newWebhookId(): string {
  return `webhook-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 8)}`;
}

function endpointLabel(endpoint: WebhookEndpoint, index: number): string {
  const name = endpoint.name?.trim();
  return name ? name : `#${index + 1}`;
}

type OutcomeLike = {
  status: string;
  detail?: string | null;
};

function channelMessage(
  outcome: OutcomeLike,
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
  // Raw per-endpoint timeout text so an in-progress edit like "1" → "" is not
  // forced back into a valid number while typing.
  const [timeoutTexts, setTimeoutTexts] = useState<Record<string, string>>({});
  const [isTesting, setIsTesting] = useState(false);
  const [testMessage, setTestMessage] = useState<string | null>(null);

  const selectedEvents = new Set(settings.events);

  const updateWebhooks = (webhooks: WebhookEndpoint[]) =>
    onChange({ ...settings, webhooks });

  const updateEndpoint = (id: string, patch: Partial<WebhookEndpoint>) =>
    updateWebhooks(
      settings.webhooks.map((endpoint) =>
        endpoint.id === id ? { ...endpoint, ...patch } : endpoint,
      ),
    );

  const addEndpoint = () =>
    updateWebhooks([
      ...settings.webhooks,
      {
        id: newWebhookId(),
        name: null,
        url: "",
        timeoutSeconds: DEFAULT_WEBHOOK_TIMEOUT_SECONDS,
        template: null,
        enabled: true,
      },
    ]);

  const removeEndpoint = (id: string) =>
    updateWebhooks(settings.webhooks.filter((endpoint) => endpoint.id !== id));

  const timeoutText = (endpoint: WebhookEndpoint) =>
    timeoutTexts[endpoint.id] ?? String(endpoint.timeoutSeconds);

  const commitTimeout = (endpoint: WebhookEndpoint, value: string) => {
    setTimeoutTexts((previous) => ({ ...previous, [endpoint.id]: value }));
    const parsed = Number.parseInt(value, 10);
    if (Number.isInteger(parsed) && parsed >= 1 && parsed <= 60) {
      updateEndpoint(endpoint.id, { timeoutSeconds: parsed });
    }
  };

  const isTimeoutValid = (endpoint: WebhookEndpoint) => {
    const parsed = Number.parseInt(timeoutText(endpoint), 10);
    return Number.isInteger(parsed) && parsed >= 1 && parsed <= 60;
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
      // Tests the current form draft, so toggling a channel or editing an
      // endpoint works without saving first.
      const result = await sendTestNotification(settings);
      const lines = [
        channelMessage(result.toast, {
          sent: t.notificationSettings.testToastSent,
          disabled: t.notificationSettings.testToastDisabled,
          failed: t.notificationSettings.testToastFailed,
        }),
      ];
      if (settings.webhookEnabled && result.webhooks.length === 0) {
        lines.push(t.notificationSettings.testWebhookNone);
      }
      for (const endpoint of result.webhooks) {
        lines.push(
          `${endpoint.label}: ${channelMessage(endpoint, {
            sent: t.notificationSettings.testWebhookSent(endpoint.statusCode ?? null),
            disabled: t.notificationSettings.testWebhookDisabled,
            failed: t.notificationSettings.testWebhookFailed,
          })}`,
        );
      }
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
        <div className="settings-field">
          <span>{t.notificationSettings.webhookEndpointsLabel}</span>
          {settings.webhooks.length === 0 ? (
            <span className="settings-hint">{t.notificationSettings.webhookEmptyHint}</span>
          ) : null}
          {settings.webhooks.map((endpoint, index) => (
            <div
              className="notification-webhook-endpoint"
              key={endpoint.id}
              data-testid={`notification-webhook-endpoint-${index}`}
            >
              <div className="notification-webhook-endpoint__header">
                <label className="checkbox-row settings-toggle-row">
                  <input
                    type="checkbox"
                    checked={endpoint.enabled}
                    data-testid={`notification-webhook-endpoint-enabled-${index}`}
                    onChange={(event) =>
                      updateEndpoint(endpoint.id, { enabled: event.currentTarget.checked })
                    }
                  />
                  {endpointLabel(endpoint, index)}
                </label>
                <button
                  type="button"
                  className="button-secondary"
                  data-testid={`notification-webhook-remove-${index}`}
                  onClick={() => removeEndpoint(endpoint.id)}
                >
                  {t.notificationSettings.webhookRemoveButton}
                </button>
              </div>
              <div className="settings-grid notification-webhook-grid">
                <label>
                  {t.notificationSettings.webhookName}
                  <input
                    type="text"
                    data-testid={`notification-webhook-name-${index}`}
                    placeholder={t.notificationSettings.webhookNamePlaceholder}
                    value={endpoint.name ?? ""}
                    onChange={(event) =>
                      updateEndpoint(endpoint.id, {
                        name: event.currentTarget.value.trim() || null,
                      })
                    }
                  />
                </label>
                <label>
                  {t.notificationSettings.webhookUrl}
                  <input
                    type="text"
                    data-testid={`notification-webhook-url-${index}`}
                    placeholder={t.notificationSettings.webhookUrlPlaceholder}
                    value={endpoint.url}
                    onChange={(event) =>
                      updateEndpoint(endpoint.id, { url: event.currentTarget.value.trim() })
                    }
                  />
                </label>
                <label>
                  {t.notificationSettings.webhookTimeout}
                  <input
                    type="number"
                    min={1}
                    max={60}
                    data-testid={`notification-webhook-timeout-${index}`}
                    value={timeoutText(endpoint)}
                    onChange={(event) => commitTimeout(endpoint, event.currentTarget.value)}
                  />
                  {!isTimeoutValid(endpoint) ? (
                    <span className="field-error" role="alert">
                      {t.notificationSettings.webhookTimeoutError}
                    </span>
                  ) : null}
                </label>
              </div>
              <label>
                {t.notificationSettings.webhookTemplate}
                <textarea
                  rows={3}
                  spellCheck={false}
                  data-testid={`notification-webhook-template-${index}`}
                  placeholder={t.notificationSettings.webhookTemplatePlaceholder}
                  value={endpoint.template ?? ""}
                  onChange={(event) =>
                    updateEndpoint(endpoint.id, {
                      template: event.currentTarget.value.trim() || null,
                    })
                  }
                />
              </label>
            </div>
          ))}
          <div className="settings-actions settings-actions--inline">
            <button
              type="button"
              className="button-secondary"
              data-testid="notification-webhook-add"
              onClick={addEndpoint}
            >
              {t.notificationSettings.webhookAddButton}
            </button>
            <button
              type="button"
              className="button-secondary"
              data-testid="notification-webhook-template-guide"
              onClick={() => void openWebhookTemplateGuide()}
            >
              {t.notificationSettings.webhookTemplateGuideButton}
            </button>
          </div>
          <span className="settings-hint">{t.notificationSettings.webhookTemplateHint}</span>
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
