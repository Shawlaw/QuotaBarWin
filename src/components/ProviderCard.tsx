import type { ProviderSnapshot } from "../types";
import { findLowestWindowPerProvider, formatResetCountdown } from "../lib/forecast";
import { formatPercent } from "../lib/format";
import { ProgressBar } from "./ProgressBar";

type ProviderCardProps = {
  provider: ProviderSnapshot;
  lowQuotaWarningThreshold?: number;
};

export function ProviderCard({ provider, lowQuotaWarningThreshold = 20 }: ProviderCardProps) {
  const forecast = findLowestWindowPerProvider(provider);

  return (
    <article className="provider-card">
      <div className="provider-card__header">
        <div>
          <h2>{provider.name}</h2>
          <p>{provider.source} provider</p>
        </div>
        <div className="provider-card__badges">
          {forecast ? <span className="bottleneck-badge">Bottleneck</span> : null}
          {forecast ? (
            <span className={`alert-badge alert-badge--${forecast.alertLevel}`}>
              {forecast.alertLevel}
            </span>
          ) : null}
          <span className={`status status--${provider.status}`}>{provider.status}</span>
        </div>
      </div>
      {provider.error ? <p className="provider-error">{provider.error}</p> : null}
      {forecast ? (
        <p className="forecast-suggestion">
          {forecast.resetLabel} · {forecast.suggestion}
        </p>
      ) : null}
      <div className="window-list">
        {provider.windows.map((window) => (
          <section className="quota-window" key={window.id}>
            <div className="quota-window__meta">
              <strong>
                {window.label}
                {forecast?.window.id === window.id ? (
                  <span className="inline-bottleneck">Bottleneck</span>
                ) : null}
              </strong>
              <span
                className={
                  window.remainingPercent !== null &&
                  window.remainingPercent <= lowQuotaWarningThreshold
                    ? "quota-window__warning"
                    : undefined
                }
              >
                {formatPercent(window.usedPercent)} used / {formatPercent(window.remainingPercent)} remaining
              </span>
            </div>
            {window.resetAt || window.resetText ? (
              <div className="quota-window__details">
                <span>{formatResetCountdown(window)}</span>
              </div>
            ) : null}
            <ProgressBar percent={window.remainingPercent} label={`${window.label} remaining`} />
          </section>
        ))}
      </div>
    </article>
  );
}
