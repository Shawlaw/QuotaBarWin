import type { ForecastWindow } from "../lib/forecast";
import { formatPercent } from "../lib/format";

type SummaryProps = {
  globalLowest: ForecastWindow | null;
};

export function Summary({ globalLowest }: SummaryProps) {
  if (!globalLowest) {
    return null;
  }

  return (
    <section className="summary" aria-label="Quota summary">
      <strong>Lowest quota alert</strong>
      <span>
        Lowest remaining across all providers: {globalLowest.providerName} ·{" "}
        {globalLowest.window.label} · {formatPercent(globalLowest.window.remainingPercent)} left
      </span>
    </section>
  );
}
