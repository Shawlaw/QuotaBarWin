import type { ProviderSnapshot } from "../types";
import { formatPercent } from "../lib/format";
import { ProgressBar } from "./ProgressBar";

type ProviderCardProps = {
  provider: ProviderSnapshot;
};

export function ProviderCard({ provider }: ProviderCardProps) {
  return (
    <article className="provider-card">
      <div className="provider-card__header">
        <div>
          <h2>{provider.name}</h2>
          <p>{provider.source} provider</p>
        </div>
        <span className={`status status--${provider.status}`}>{provider.status}</span>
      </div>
      <div className="window-list">
        {provider.windows.map((window) => (
          <section className="quota-window" key={window.id}>
            <div className="quota-window__meta">
              <strong>{window.label}</strong>
              <span>{formatPercent(window.remainingPercent)} remaining</span>
            </div>
            <ProgressBar percent={window.remainingPercent} label={`${window.label} remaining`} />
          </section>
        ))}
      </div>
    </article>
  );
}
