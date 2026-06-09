import { useState } from "react";
import type { ProviderDiagnostics, ProviderSnapshot, QuotaWindow } from "../types";
import {
  calculateProviderStatus,
  displayPercentForWindow,
  formatDisplayValue,
  formatQuotaReset,
  formatShortDateTime,
  windowStatus
} from "../lib/providerStatus";
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
  const [statusOpen, setStatusOpen] = useState(false);
  const [expanded, setExpanded] = useState(false);
  const refreshedAt = provider.updatedAt ?? provider.diagnostics?.checkedAt ?? null;
  const effectiveStatus = calculateProviderStatus(provider, lowQuotaWarningThreshold);
  const statusLabel = `status ${effectiveStatus}`;
  const canOpenStatus = effectiveStatus === "warning" || effectiveStatus === "error";
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
              ? `Last updated ${formatShortDateTime(refreshedAt) ?? "recently"}`
              : `${provider.source} provider`}
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
              {isRefreshing ? "Refreshing" : "Refresh"}
            </button>
          ) : null}
        </div>
      </div>
      {provider.error ? <p className="provider-error">Refresh failed · {provider.error}</p> : null}
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
          <p className="provider-empty">No quota windows reported yet.</p>
        )}
      </div>
      {hiddenWindowCount > 0 ? (
        <button
          type="button"
          className="button-ghost quota-more-button"
          onClick={() => setExpanded((current) => !current)}
        >
          {expanded ? "Show less" : `+ ${hiddenWindowCount} more quota windows`}
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
  const displayedPercent = displayPercentForWindow(window, displayMode);
  const status = windowStatus(window, lowQuotaWarningThreshold);
  const resetText = formatQuotaReset(window);

  return (
    <section
      className={`quota-window quota-window--${status}`}
      data-testid={`quota-row-${providerId}-${window.id}`}
    >
      <div className="quota-window__meta">
        <strong>{window.label}</strong>
        <span className={status === "warning" ? "quota-window__warning" : undefined}>
          {formatDisplayValue(window, displayMode)}
        </span>
      </div>
      <div className="quota-window__details">
        {resetText ? <span>{resetText}</span> : <span>No reset time</span>}
      </div>
      {displayedPercent !== null ? (
        <ProgressBar
          percent={displayedPercent}
          opacityPercent={displayedPercent}
          label={`${providerName} ${window.label} ${displayMode}`}
          tone={status === "warning" ? "warning" : "normal"}
        />
      ) : (
        <div className="quota-window__no-progress">Progress unavailable</div>
      )}
    </section>
  );
}

function StatusDetails({
  error,
  diagnostics
}: {
  error?: string | null;
  diagnostics?: ProviderDiagnostics | null;
}) {
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
              <dt>Exit code</dt>
              <dd>{diagnostics.exitCode}</dd>
            </>
          ) : null}
          {diagnostics.durationMs !== null && diagnostics.durationMs !== undefined ? (
            <>
              <dt>Duration</dt>
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
