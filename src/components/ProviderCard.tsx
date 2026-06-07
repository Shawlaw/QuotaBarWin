import type { ProviderSnapshot } from "../types";
import { formatPercent } from "../lib/format";
import { ProgressBar } from "./ProgressBar";

type ProviderCardProps = {
  provider: ProviderSnapshot;
  lowQuotaWarningThreshold?: number;
};

export function ProviderCard({ provider, lowQuotaWarningThreshold = 20 }: ProviderCardProps) {
  return (
    <article className="provider-card">
      <div className="provider-card__header">
        <div>
          <h2>{provider.name}</h2>
          <p>{provider.source} provider</p>
        </div>
        <span className={`status status--${provider.status}`}>{provider.status}</span>
      </div>
      {provider.error ? <p className="provider-error">{provider.error}</p> : null}
      <div className="window-list">
        {provider.windows.map((window) => (
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
                {formatPercent(window.usedPercent)} used / {formatPercent(window.remainingPercent)} remaining
              </span>
            </div>
            <ProgressBar percent={window.remainingPercent} label={`${window.label} remaining`} />
          </section>
        ))}
      </div>
    </article>
  );
}
