import { useState } from "react";
import type { ProviderDiagnostics, ProviderSnapshot } from "../types";
import { formatResetCountdown } from "../lib/forecast";
import { formatPercent } from "../lib/format";
import { ProgressBar } from "./ProgressBar";

type ProviderCardProps = {
  provider: ProviderSnapshot;
  displayMode?: "remaining" | "used";
  lowQuotaWarningThreshold?: number;
  isRefreshing?: boolean;
  onRefresh?: () => void;
};

export function ProviderCard({
  provider,
  displayMode = "remaining",
  lowQuotaWarningThreshold = 20,
  isRefreshing = false,
  onRefresh
}: ProviderCardProps) {
  const [statusOpen, setStatusOpen] = useState(false);
  const refreshedAt = provider.updatedAt ?? provider.diagnostics?.checkedAt ?? null;
  const statusLabel = `status ${provider.status}`;
  const canOpenStatus = provider.status === "warning" || provider.status === "error";

  return (
    <article className="provider-card">
      <div className="provider-card__header">
        <div>
          <h2>{provider.name}</h2>
          <p>
            {refreshedAt
              ? `Last refresh ${new Date(refreshedAt).toLocaleString()}`
              : `${provider.source} provider`}
          </p>
        </div>
        <div className="provider-card__badges">
          {onRefresh ? (
            <button
              type="button"
              className="button-secondary button-compact"
              onClick={onRefresh}
              disabled={isRefreshing}
            >
              {isRefreshing ? "Refreshing" : "Refresh provider"}
            </button>
          ) : null}
          {canOpenStatus ? (
            <button
              type="button"
              className={`status status--${provider.status} status-button`}
              onClick={() => setStatusOpen((current) => !current)}
            >
              {statusLabel}
            </button>
          ) : null}
          {!canOpenStatus ? (
            <span className={`status status--${provider.status}`}>{statusLabel}</span>
          ) : null}
        </div>
      </div>
      {provider.error ? <p className="provider-error">{provider.error}</p> : null}
      {statusOpen ? <StatusDetails error={provider.error} diagnostics={provider.diagnostics} /> : null}
      <div className="window-list">
        {provider.windows.map((window) => {
          const displayedPercent =
            displayMode === "used" ? window.usedPercent : window.remainingPercent;
          const displayedLabel = displayMode === "used" ? "used" : "remaining";
          const opacityPercent =
            displayMode === "used" && window.usedPercent !== null
              ? 100 - window.usedPercent
              : window.remainingPercent;

          return (
            <section className="quota-window" key={window.id}>
              <div className="quota-window__meta">
                <strong>{window.label}</strong>
                <span
                  className={
                    window.remainingPercent !== null &&
                    window.remainingPercent <= lowQuotaWarningThreshold
                      ? "quota-window__warning"
                      : undefined
                  }
                >
                  {formatPercent(displayedPercent)} {displayedLabel}
                </span>
              </div>
              {window.resetAt || window.resetText ? (
                <div className="quota-window__details">
                  <span>{formatResetCountdown(window)}</span>
                </div>
              ) : null}
              <ProgressBar
                percent={displayedPercent}
                opacityPercent={opacityPercent}
                label={`${window.label} ${displayedLabel}`}
              />
            </section>
          );
        })}
      </div>
    </article>
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
