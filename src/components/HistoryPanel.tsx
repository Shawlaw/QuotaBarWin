import { useState } from "react";
import type { QuotaEvent } from "../types";
import { useI18n } from "../i18n";
import { eventCategory, eventMessage, formatEventTime } from "../lib/quotaEvents";

type EventFilter = "all" | "quota" | "provider" | "app";

type HistoryPanelProps = {
  events: QuotaEvent[] | null;
  loadFailed: boolean;
  onRefresh: () => void;
  onClear: () => void;
};

function severityClass(severity: string): string {
  switch (severity) {
    case "positive":
      return " event-item--positive";
    case "warning":
      return " event-item--warning";
    case "error":
      return " event-item--error";
    default:
      return " event-item--info";
  }
}

export function HistoryPanel({ events, loadFailed, onRefresh, onClear }: HistoryPanelProps) {
  const { t } = useI18n();
  const [filter, setFilter] = useState<EventFilter>("all");

  const filters: { key: EventFilter; label: string }[] = [
    { key: "all", label: t.events.filterAll },
    { key: "quota", label: t.events.filterQuota },
    { key: "provider", label: t.events.filterProvider },
    { key: "app", label: t.events.filterApp },
  ];

  const visibleEvents =
    events?.filter((event) => filter === "all" || eventCategory(event.eventType) === filter) ?? [];

  return (
    <section className="history-page" aria-label={t.events.title} data-testid="history-page">
      <div className="history-header">
        <div className="history-header__text">
          <h2>{t.events.title}</h2>
          <p className="history-subtitle">{t.events.subtitle}</p>
        </div>
        <div className="history-actions">
          <button
            type="button"
            className="button-secondary"
            data-testid="history-refresh"
            onClick={onRefresh}
          >
            {t.header.refresh}
          </button>
          <button
            type="button"
            className="button-danger"
            data-testid="history-clear"
            disabled={!events || events.length === 0}
            onClick={() => {
              if (window.confirm(t.events.clearConfirm)) {
                onClear();
              }
            }}
          >
            {t.events.clear}
          </button>
        </div>
      </div>
      <div className="history-filters" role="group" aria-label={t.events.title}>
        {filters.map((item) => (
          <button
            key={item.key}
            type="button"
            className={filter === item.key ? "history-filter-chip history-filter-chip--active" : "history-filter-chip"}
            data-testid={`history-filter-${item.key}`}
            onClick={() => setFilter(item.key)}
          >
            {item.label}
          </button>
        ))}
      </div>
      {loadFailed ? (
        <div className="history-empty" role="alert" data-testid="history-load-failed">
          {t.events.loadFailed}
        </div>
      ) : events === null ? (
        <div className="history-empty" data-testid="history-loading">
          {t.settings.loading}
        </div>
      ) : visibleEvents.length === 0 ? (
        <div className="history-empty" data-testid="history-empty">
          {t.events.empty}
        </div>
      ) : (
        <ul className="history-list" data-testid="history-list">
          {visibleEvents.map((event) => (
            <li
              key={event.id}
              className={`history-item${severityClass(String(event.severity))}`}
              data-testid="history-item"
            >
              <span className="history-item__dot" aria-hidden="true" />
              <div className="history-item__body">
                <span className="history-item__message">{eventMessage(t, event)}</span>
                <time className="history-item__time" dateTime={event.occurredAt} title={event.occurredAt}>
                  {formatEventTime(event.occurredAt)}
                </time>
              </div>
            </li>
          ))}
        </ul>
      )}
    </section>
  );
}
