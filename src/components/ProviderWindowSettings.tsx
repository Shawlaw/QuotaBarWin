import { useState } from "react";
import { useI18n } from "../i18n";
import type { QuotaWindow, RemoteProviderConfig } from "../types";

export type WindowConfigProvider = RemoteProviderConfig;

export type WindowDisplayPatch = {
  windowLabelOverrides?: Record<string, string>;
  visibleWindowIds?: string[];
};

type WindowRow = {
  id: string;
  defaultLabel: string;
  fromSnapshot: boolean;
};

type ProviderWindowSettingsProps = {
  provider: WindowConfigProvider;
  snapshotWindows?: QuotaWindow[];
  onChange: (patch: WindowDisplayPatch) => void;
};

function visibleWindowsToText(windowIds: string[] | undefined): string {
  return (windowIds ?? []).join("\n");
}

function textToVisibleWindows(text: string): string[] {
  return text
    .split(/\r?\n/)
    .map((line) => line.trim())
    .filter(Boolean);
}

function labelOverridesToText(overrides: Record<string, string> | undefined): string {
  return Object.entries(overrides ?? {})
    .map(([id, label]) => `${id}=${label}`)
    .join("\n");
}

function textToLabelOverrides(text: string): Record<string, string> {
  return Object.fromEntries(
    text
      .split(/\r?\n/)
      .map((line) => line.trim())
      .filter(Boolean)
      .map((line) => {
        const separator = line.indexOf("=");
        if (separator === -1) {
          return null;
        }
        return [line.slice(0, separator).trim(), line.slice(separator + 1).trim()] as const;
      })
      .filter((entry): entry is readonly [string, string] =>
        Boolean(entry && entry[0].length > 0 && entry[1].length > 0)
      )
  );
}

function uniqueIds(ids: string[]): string[] {
  return Array.from(new Set(ids.filter(Boolean)));
}

function buildWindowRows(
  snapshotWindows: QuotaWindow[] | undefined,
  provider: WindowConfigProvider
): WindowRow[] {
  const snapshotRows =
    snapshotWindows?.map((window) => ({
      id: window.id,
      defaultLabel: window.label,
      fromSnapshot: true
    })) ?? [];
  const rowsById = new Map(snapshotRows.map((row) => [row.id, row]));

  for (const id of uniqueIds([
    ...(provider.visibleWindowIds ?? []),
    ...Object.keys(provider.windowLabelOverrides ?? {})
  ])) {
    if (!rowsById.has(id)) {
      rowsById.set(id, {
        id,
        defaultLabel: id,
        fromSnapshot: false
      });
    }
  }

  return Array.from(rowsById.values());
}

function orderedRows(rows: WindowRow[], visibleWindowIds: string[] | undefined): WindowRow[] {
  const configuredIds = uniqueIds(visibleWindowIds ?? []);
  if (configuredIds.length === 0) {
    return rows;
  }

  const rowById = new Map(rows.map((row) => [row.id, row]));
  const configuredRows = configuredIds.flatMap((id) => {
    const row = rowById.get(id);
    return row ? [row] : [];
  });
  const remainingRows = rows.filter((row) => !configuredIds.includes(row.id));

  return [...configuredRows, ...remainingRows];
}

export function ProviderWindowSettings({
  provider,
  snapshotWindows,
  onChange
}: ProviderWindowSettingsProps) {
  const { t } = useI18n();
  const [labelOverrideDraft, setLabelOverrideDraft] = useState<string | null>(null);
  const [visibleWindowDraft, setVisibleWindowDraft] = useState<string | null>(null);
  const defaultRows = buildWindowRows(snapshotWindows, provider);
  const rows = orderedRows(defaultRows, provider.visibleWindowIds);
  const allRowIds = rows.map((row) => row.id);
  const defaultRowIds = defaultRows.map((row) => row.id);
  const visibleIds =
    provider.visibleWindowIds && provider.visibleWindowIds.length > 0
      ? uniqueIds(provider.visibleWindowIds).filter((id) => allRowIds.includes(id))
      : allRowIds;
  const visibleIdSet = new Set(visibleIds);
  const hasSnapshotWindows = Boolean(snapshotWindows?.length);

  function setVisibleIds(nextVisibleIds: string[]) {
    setVisibleWindowDraft(null);
    onChange({ visibleWindowIds: uniqueIds(nextVisibleIds) });
  }

  function setLabelOverride(windowId: string, value: string) {
    setLabelOverrideDraft(null);
    const nextOverrides = { ...(provider.windowLabelOverrides ?? {}) };
    const nextValue = value.trim();
    if (nextValue.length === 0) {
      delete nextOverrides[windowId];
    } else {
      nextOverrides[windowId] = nextValue;
    }
    onChange({ windowLabelOverrides: nextOverrides });
  }

  function toggleVisible(windowId: string, checked: boolean) {
    const nextVisibleIds = checked
      ? [...visibleIds, windowId]
      : visibleIds.filter((id) => id !== windowId);
    setVisibleIds(nextVisibleIds);
  }

  function moveVisible(windowId: string, direction: -1 | 1) {
    const fromIndex = visibleIds.indexOf(windowId);
    const toIndex = fromIndex + direction;
    if (fromIndex === -1 || toIndex < 0 || toIndex >= visibleIds.length) {
      return;
    }

    const nextVisibleIds = [...visibleIds];
    const [id] = nextVisibleIds.splice(fromIndex, 1);
    nextVisibleIds.splice(toIndex, 0, id);
    setVisibleIds(nextVisibleIds);
  }

  function resetDefaultOrder() {
    setVisibleIds(defaultRowIds.filter((id) => visibleIdSet.has(id)));
  }

  return (
    <section className="window-settings args-field" aria-label={t.settings.windowDisplay}>
      <div className="window-settings__header">
        <h4>{t.settings.windowDisplay}</h4>
        <div className="window-settings__actions">
          <button type="button" className="button-secondary button-compact" onClick={() => setVisibleIds([])}>
            {t.settings.showAllWindows}
          </button>
          <button
            type="button"
            className="button-secondary button-compact"
            onClick={() => {
              setLabelOverrideDraft(null);
              onChange({ windowLabelOverrides: {} });
            }}
          >
            {t.settings.resetWindowNames}
          </button>
          <button type="button" className="button-secondary button-compact" onClick={resetDefaultOrder}>
            {t.settings.resetWindowOrder}
          </button>
        </div>
      </div>

      {!hasSnapshotWindows ? <p className="settings-hint">{t.settings.windowDisplayNoSnapshot}</p> : null}

      {rows.length > 0 ? (
        <div className="window-settings__table">
          <div className="window-settings__head" aria-hidden="true">
            <span>{t.settings.show}</span>
            <span>{t.settings.windowId}</span>
            <span>{t.settings.defaultLabel}</span>
            <span>{t.settings.customLabel}</span>
            <span>{t.settings.order}</span>
          </div>
          {rows.map((row) => {
            const visibleIndex = visibleIds.indexOf(row.id);
            const isVisible = visibleIdSet.has(row.id);
            return (
              <div className="window-settings__row" key={row.id}>
                <label className="window-settings__checkbox">
                  <input
                    type="checkbox"
                    checked={isVisible}
                    aria-label={t.settings.showWindow(row.id)}
                    onChange={(event) => toggleVisible(row.id, event.currentTarget.checked)}
                  />
                </label>
                <code title={row.id}>{row.id}</code>
                <span className={row.fromSnapshot ? undefined : "window-settings__fallback-label"}>
                  {row.defaultLabel}
                </span>
                <label className="window-settings__custom-label">
                  <span>{t.settings.customLabel}</span>
                  <input
                    aria-label={t.settings.customLabelFor(row.id)}
                    placeholder={row.defaultLabel}
                    value={provider.windowLabelOverrides?.[row.id] ?? ""}
                    onChange={(event) => setLabelOverride(row.id, event.currentTarget.value)}
                  />
                </label>
                <div className="window-settings__order-actions">
                  <button
                    type="button"
                    className="button-secondary button-compact"
                    disabled={!isVisible || visibleIndex <= 0}
                    aria-label={t.settings.moveWindowUp(row.id)}
                    onClick={() => moveVisible(row.id, -1)}
                  >
                    {t.settings.up}
                  </button>
                  <button
                    type="button"
                    className="button-secondary button-compact"
                    disabled={!isVisible || visibleIndex === -1 || visibleIndex >= visibleIds.length - 1}
                    aria-label={t.settings.moveWindowDown(row.id)}
                    onClick={() => moveVisible(row.id, 1)}
                  >
                    {t.settings.down}
                  </button>
                </div>
              </div>
            );
          })}
        </div>
      ) : (
        <p className="settings-empty">{t.settings.windowDisplayNoRows}</p>
      )}

      <details className="window-settings__advanced" open={rows.length === 0}>
        <summary>{t.settings.advancedWindowText}</summary>
        <div className="window-settings__advanced-fields">
          <label className="args-field">
            {t.settings.windowLabelOverrides}
            <textarea
              rows={4}
              placeholder={t.settings.windowLabelOverridesPlaceholder}
              value={labelOverrideDraft ?? labelOverridesToText(provider.windowLabelOverrides)}
              onChange={(event) => {
                setLabelOverrideDraft(event.currentTarget.value);
                onChange({
                  windowLabelOverrides: textToLabelOverrides(event.currentTarget.value)
                });
              }}
            />
          </label>
          <label className="args-field">
            {t.settings.displayedWindows}
            <textarea
              rows={3}
              placeholder={t.settings.displayedWindowsPlaceholder}
              value={visibleWindowDraft ?? visibleWindowsToText(provider.visibleWindowIds)}
              onChange={(event) => {
                setVisibleWindowDraft(event.currentTarget.value);
                onChange({
                  visibleWindowIds: textToVisibleWindows(event.currentTarget.value)
                });
              }}
            />
          </label>
        </div>
      </details>
    </section>
  );
}
