import { en, type I18nCatalog } from "../i18n/catalog";

export function clampPercent(percent: number): number {
  if (Number.isNaN(percent)) {
    return 0;
  }

  return Math.min(100, Math.max(0, percent));
}

export function formatPercent(percent: number | null, catalog: I18nCatalog = en): string {
  if (percent === null) {
    return catalog.format.unknown;
  }

  return `${Math.round(clampPercent(percent))}%`;
}
