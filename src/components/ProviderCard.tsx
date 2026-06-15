import { useState } from "react";
import type { ProviderDiagnostics, ProviderSnapshot, QuotaWindow } from "../types";
import {
  calculateProviderStatus,
  displayPercentForWindow,
  formatDisplayValue,
  formatQuotaReset,
  remainingAmountForWindow,
  formatShortDateTime,
  windowStatus
} from "../lib/providerStatus";
import { useI18n } from "../i18n";
import { ProgressBar } from "./ProgressBar";

type ProviderCardProps = {
  provider: ProviderSnapshot;
  displayMode?: "remaining" | "used";
  lowQuotaWarningThreshold?: number;
  isRefreshing?: boolean;
  onRefresh?: () => void;
};

const MAX_COLLAPSED_WINDOWS = 4;

export function ProviderCard({
  provider,
  displayMode = "remaining",
  lowQuotaWarningThreshold = 20,
  isRefreshing = false,
  onRefresh
}: ProviderCardProps) {
  const { t } = useI18n();
  const [statusOpen, setStatusOpen] = useState(false);
  const [expanded, setExpanded] = useState(false);
  const refreshedAt = provider.updatedAt ?? provider.diagnostics?.checkedAt ?? null;
  const effectiveStatus = calculateProviderStatus(provider, lowQuotaWarningThreshold);
  const statusLabel = t.providerCard.statusLabel(effectiveStatus);
  const canOpenStatus = effectiveStatus === "warning" || effectiveStatus === "error" || effectiveStatus === "stale";
  const hiddenWindowCount = Math.max(0, provider.windows.length - MAX_COLLAPSED_WINDOWS);
  const visibleWindows = expanded
    ? provider.windows
    : provider.windows.slice(0, MAX_COLLAPSED_WINDOWS);

  return (
    <article className={`provider-card provider-card--${effectiveStatus}`} data-testid={`provider-card-${provider.id}`}>
      <div className="provider-card__header">
        <div>
          <h2>{provider.name}</h2>
          <p>
            {refreshedAt
              ? t.providerCard.lastUpdated(
                  formatShortDateTime(refreshedAt) ?? t.globalStatus.recently
                )
              : t.providerCard.providerSource(provider.source)}
          </p>
        </div>
        <div className="provider-card__badges">
          {canOpenStatus ? (
            <button
              type="button"
              className={`status status--${effectiveStatus} status-button`}
              onClick={() => setStatusOpen((current) => !current)}
              data-testid={`provider-status-${provider.id}`}
            >
              {statusLabel}
            </button>
          ) : (
            <span className={`status status--${effectiveStatus}`} data-testid={`provider-status-${provider.id}`}>
              {statusLabel}
            </span>
          )}
          {onRefresh ? (
            <button
              type="button"
              className="button-secondary button-compact"
              onClick={onRefresh}
              disabled={isRefreshing}
              data-testid={`provider-refresh-${provider.id}`}
            >
              {isRefreshing ? t.providerCard.refreshing : t.providerCard.refresh}
            </button>
          ) : null}
        </div>
      </div>
      {provider.error ? (
        <p className="provider-error">
          {effectiveStatus === "stale"
            ? t.providerCard.showingCachedDataPrefix
            : t.providerCard.refreshFailedPrefix}
          {provider.error}
        </p>
      ) : null}
      {statusOpen ? <StatusDetails error={provider.error} diagnostics={provider.diagnostics} /> : null}
      <div className="window-list">
        {visibleWindows.length > 0 ? (
          visibleWindows.map((window) => (
            <QuotaWindowRow
              key={window.id}
              providerId={provider.id}
              providerName={provider.name}
              window={window}
              displayMode={displayMode}
              lowQuotaWarningThreshold={lowQuotaWarningThreshold}
            />
          ))
        ) : (
          <p className="provider-empty">{t.providerCard.noQuotaWindows}</p>
        )}
      </div>
      {hiddenWindowCount > 0 ? (
        <button
          type="button"
          className="button-ghost quota-more-button"
          onClick={() => setExpanded((current) => !current)}
        >
          {expanded ? t.providerCard.showLess : t.providerCard.moreQuotaWindows(hiddenWindowCount)}
        </button>
      ) : null}
    </article>
  );
}

function QuotaWindowRow({
  providerId,
  providerName,
  window,
  displayMode,
  lowQuotaWarningThreshold
}: {
  providerId: string;
  providerName: string;
  window: QuotaWindow;
  displayMode: "remaining" | "used";
  lowQuotaWarningThreshold: number;
}) {
  const { t } = useI18n();
  const displayedPercent = displayPercentForWindow(window, displayMode);
  const amountDetail = formatAmountDetail(window);
  const opacityPercent =
    window.remainingPercent !== null && window.remainingPercent !== undefined
      ? window.remainingPercent
      : window.usedPercent !== null && window.usedPercent !== undefined
        ? 100 - window.usedPercent
        : null;
  const status = windowStatus(window, lowQuotaWarningThreshold);
  const resetText = formatQuotaReset(window, new Date(), t);

  return (
    <section
      className={`quota-window quota-window--${status}`}
      data-testid={`quota-row-${providerId}-${window.id}`}
    >
      <div className="quota-window__meta">
        <strong>{window.label}</strong>
        <span className={status === "warning" ? "quota-window__warning" : undefined}>
          {formatDisplayValue(window, displayMode, t)}
        </span>
      </div>
      <div className="quota-window__details">
        {amountDetail ? <span>{amountDetail}</span> : null}
        {resetText ? <span>{resetText}</span> : amountDetail ? null : <span>{t.providerCard.noResetTime}</span>}
      </div>
      {displayedPercent !== null ? (
        <ProgressBar
          percent={displayedPercent}
          opacityPercent={opacityPercent}
          label={`${providerName} ${window.label} ${displayMode}`}
          tone={status === "warning" ? "warning" : "normal"}
        />
      ) : (
        <div className="quota-window__no-progress">{t.providerCard.progressUnavailable}</div>
      )}
    </section>
  );
}

function formatAmountDetail(window: QuotaWindow): string | null {
  const unit = window.unit?.trim();
  const shouldShowAmount =
    Boolean(unit && unit !== "percent") ||
    window.warningRemaining !== null && window.warningRemaining !== undefined;
  if (!shouldShowAmount) {
    return null;
  }

  const remaining = remainingAmountForWindow(window);
  if (remaining === null) {
    return null;
  }

  const suffix = unit ? ` ${unit}` : "";
  const parts = [`Remaining ${formatAmount(remaining)}${suffix}`];
  if (typeof window.limit === "number" && Number.isFinite(window.limit)) {
    parts[0] += ` / ${formatAmount(window.limit)}${suffix}`;
  }
  if (
    window.warningRemaining !== null &&
    window.warningRemaining !== undefined &&
    Number.isFinite(window.warningRemaining)
  ) {
    parts.push(`warning ${formatAmount(window.warningRemaining)}${suffix}`);
  }

  return parts.join(" · ");
}

function formatAmount(value: number): string {
  return Number.isInteger(value) ? String(value) : value.toFixed(2);
}

function StatusDetails({
  error,
  diagnostics
}: {
  error?: string | null;
  diagnostics?: ProviderDiagnostics | null;
}) {
  const { t } = useI18n();
  const messages = diagnostics?.messages ?? [];

  return (
    <div className="status-details" role="status">
      {error ? <p>{error}</p> : null}
      {messages.length > 0 ? (
        <ul>
          {messages.map((message) => (
            <li key={message}>{message}</li>
          ))}
        </ul>
      ) : null}
      {diagnostics ? (
        <dl>
          {diagnostics.exitCode !== null && diagnostics.exitCode !== undefined ? (
            <>
              <dt>{t.providerCard.exitCode}</dt>
              <dd>{diagnostics.exitCode}</dd>
            </>
          ) : null}
          {diagnostics.durationMs !== null && diagnostics.durationMs !== undefined ? (
            <>
              <dt>{t.providerCard.duration}</dt>
              <dd>{diagnostics.durationMs}ms</dd>
            </>
          ) : null}
          {diagnostics.stderr ? (
            <>
              <dt>stderr</dt>
              <dd>{diagnostics.stderr}</dd>
            </>
          ) : null}
        </dl>
      ) : null}
    </div>
  );
}
