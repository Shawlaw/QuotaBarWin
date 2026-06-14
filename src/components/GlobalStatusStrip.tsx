import type { ProviderSnapshot } from "../types";
import { calculateProviderStatus, formatShortDateTime } from "../lib/providerStatus";
import { useI18n } from "../i18n";

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
  const { t } = useI18n();

  if (providers.length === 0) {
    return (
      <section className="global-status global-status--empty" data-testid="global-status-strip">
        {t.globalStatus.noProviders}
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
        {t.globalStatus.refreshFailed(error.provider.name)}
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
        {t.globalStatus.needsAttention(
          warning.provider.name,
          window ? t.globalStatus.belowThreshold(window.label, lowQuotaWarningThreshold) : undefined
        )}
      </section>
    );
  }

  if (stale) {
    return (
      <section className="global-status global-status--stale" data-testid="global-status-strip">
        {t.globalStatus.dataStale(stale.provider.name)}
      </section>
    );
  }

  return (
    <section className="global-status" data-testid="global-status-strip">
      {t.globalStatus.summary(
        providers.length,
        formatShortDateTime(refreshedAt) ?? t.globalStatus.recently,
        refreshIntervalSeconds
      )}
    </section>
  );
}
