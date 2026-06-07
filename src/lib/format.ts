export function clampPercent(percent: number): number {
  if (Number.isNaN(percent)) {
    return 0;
  }

  return Math.min(100, Math.max(0, percent));
}

export function formatPercent(percent: number | null): string {
  if (percent === null) {
    return "Unknown";
  }

  return `${Math.round(clampPercent(percent))}%`;
}
