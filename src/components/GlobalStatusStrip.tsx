import type { ProviderSnapshot } from "../types";
import { calculateProviderStatus, formatShortDateTime } from "../lib/providerStatus";

type GlobalStatusStripProps = {
  providers: ProviderSnapshot[];
  refreshIntervalSeconds?: number;
  refreshedAt?: string | null;
  lowQuotaWarningThreshold?: number;
};

export function GlobalStatusStrip({
  providers,
  refreshIntervalSeconds = 300,
  refreshedAt,
  lowQuotaWarningThreshold = 20
}: GlobalStatusStripProps) {
  if (providers.length === 0) {
    return (
      <section className="global-status global-status--empty" data-testid="global-status-strip">
        No providers configured. Add a provider in Settings.
      </section>
    );
  }

  const statuses = providers.map((provider) => ({
    provider,
    status: calculateProviderStatus(provider, lowQuotaWarningThreshold)
  }));
  const error = statuses.find(({ status }) => status === "error");
  const warning = statuses.find(({ status }) => status === "warning");
  const stale = statuses.find(({ status }) => status === "stale");

  if (error) {
    return (
      <section className="global-status global-status--error" data-testid="global-status-strip">
        {error.provider.name} refresh failed
      </section>
    );
  }

  if (warning) {
    const window = warning.provider.windows.find(
      (candidate) =>
        candidate.remainingPercent !== null &&
        candidate.remainingPercent !== undefined &&
        candidate.remainingPercent <= lowQuotaWarningThreshold
    );

    return (
      <section className="global-status global-status--warning" data-testid="global-status-strip">
        {warning.provider.name} needs attention{window ? ` · ${window.label} below ${lowQuotaWarningThreshold}%` : ""}
      </section>
    );
  }

  if (stale) {
    return (
      <section className="global-status global-status--stale" data-testid="global-status-strip">
        {stale.provider.name} data is stale
      </section>
    );
  }

  return (
    <section className="global-status" data-testid="global-status-strip">
      {providers.length} providers active · Last updated {formatShortDateTime(refreshedAt) ?? "recently"} · Auto refresh
      every {refreshIntervalSeconds}s
    </section>
  );
}
