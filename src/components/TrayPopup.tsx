import { useCallback, useEffect, useRef, useState, type MouseEvent } from "react";
import {
  getCachedSnapshot,
  getConfig,
  getTrayPopupPresentationId,
  hideCurrentWindow,
  hideTrayPopup,
  listenForSnapshotUpdates,
  listenForTrayPopupShown,
  resetTrayPopupSize,
  refreshSnapshot,
  startDraggingCurrentWindow,
  startResizingCurrentWindow
} from "../lib/api";
import {
  calculateProviderStatus,
  displayPercentForWindow,
  formatDisplayValue,
  formatQuotaReset,
  formatShortDateTime,
  windowStatus
} from "../lib/providerStatus";
import type { AppConfig, AppSnapshot, ProviderSnapshot, QuotaWindow } from "../types";
import { useI18n } from "../i18n";
import { ProgressBar } from "./ProgressBar";

type WindowRow = {
  provider: ProviderSnapshot;
  window: QuotaWindow;
};

type ProviderIssue = {
  provider: ProviderSnapshot;
  status: ProviderSnapshot["status"];
};

const issueStatusPriority: Record<ProviderSnapshot["status"], number> = {
  error: 0,
  stale: 1,
  warning: 2,
  unknown: 3,
  ok: 4
};

function windowProgressTone(
  providerStatus: ProviderSnapshot["status"],
  quotaStatus: ReturnType<typeof windowStatus>
): "normal" | "warning" | "error" {
  if (providerStatus === "error") {
    return "error";
  }

  if (providerStatus === "stale" || quotaStatus === "warning") {
    return "warning";
  }

  return "normal";
}

function orderedWindows(providers: ProviderSnapshot[]): WindowRow[] {
  return providers.flatMap((provider) => provider.windows.map((window) => ({ provider, window })));
}

function orderedProviderIssues(
  providers: ProviderSnapshot[],
  lowQuotaWarningThreshold: number
): ProviderIssue[] {
  return providers
    .map((provider) => ({
      provider,
      status: calculateProviderStatus(provider, lowQuotaWarningThreshold)
    }))
    .filter((issue) => issue.status !== "ok")
    .sort((left, right) => issueStatusPriority[left.status] - issueStatusPriority[right.status]);
}

export function TrayPopup() {
  const { t } = useI18n();
  const refreshInFlight = useRef(false);
  const lastHandledPresentationId = useRef(0);
  const [snapshot, setSnapshot] = useState<AppSnapshot | null>(null);
  const [config, setConfig] = useState<AppConfig | null>(null);
  const [isLoading, setIsLoading] = useState(false);

  const syncCachedSnapshot = useCallback(async () => {
    try {
      const cached = await getCachedSnapshot();
      if (cached) {
        setSnapshot(cached);
      }
      return cached;
    } catch {
      return null;
    }
  }, []);

  const loadSnapshot = useCallback(async () => {
    if (refreshInFlight.current) {
      return;
    }

    refreshInFlight.current = true;
    setIsLoading(true);
    try {
      await syncCachedSnapshot();
      setSnapshot(await refreshSnapshot());
    } catch {
      setSnapshot(await syncCachedSnapshot());
    } finally {
      refreshInFlight.current = false;
      setIsLoading(false);
    }
  }, [syncCachedSnapshot]);

  const refreshForPresentation = useCallback((presentationId: number) => {
    if (
      !Number.isFinite(presentationId) ||
      presentationId <= lastHandledPresentationId.current
    ) {
      return;
    }

    lastHandledPresentationId.current = presentationId;
    void loadSnapshot();
  }, [loadSnapshot]);

  const syncTrayPopupPresentation = useCallback(async () => {
    try {
      refreshForPresentation(await getTrayPopupPresentationId());
    } catch {
      // Focus is only a best-effort fallback for a missed tray-popup-shown event.
    }
  }, [refreshForPresentation]);

  useEffect(() => {
    let isMounted = true;

    async function initialize() {
      const [loadedConfig, cached] = await Promise.all([getConfig(), getCachedSnapshot()]);
      if (!isMounted) {
        return;
      }

      setConfig(loadedConfig);
      setSnapshot(cached);
      void loadSnapshot();
    }

    void initialize();

    return () => {
      isMounted = false;
    };
  }, [loadSnapshot]);

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    void listenForTrayPopupShown(refreshForPresentation).then((cleanup) => {
      unlisten = cleanup;
      void syncTrayPopupPresentation();
    });

    return () => {
      unlisten?.();
    };
  }, [refreshForPresentation, syncTrayPopupPresentation]);

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    void listenForSnapshotUpdates((updatedSnapshot) => setSnapshot(updatedSnapshot)).then((cleanup) => {
      unlisten = cleanup;
    });

    return () => {
      unlisten?.();
    };
  }, []);

  useEffect(() => {
    function onFocus() {
      void syncTrayPopupPresentation();
    }

    window.addEventListener("focus", onFocus);
    return () => window.removeEventListener("focus", onFocus);
  }, [syncTrayPopupPresentation]);

  useEffect(() => {
    function onKeyDown(event: KeyboardEvent) {
      if (event.key === "Escape") {
        void hideTrayPopup().catch(() => hideCurrentWindow());
      }
    }

    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, []);

  const providers = snapshot?.providers ?? [];
  const lowQuotaWarningThreshold = config?.lowQuotaWarningThreshold ?? 20;
  const displayMode = config?.displayMode ?? "remaining";
  const rows = orderedWindows(providers);
  const providerIssues = orderedProviderIssues(providers, lowQuotaWarningThreshold);
  const primaryIssue = providerIssues[0] ?? null;
  const refreshedText = formatShortDateTime(snapshot?.refreshedAt);

  function onTitleMouseDown(event: MouseEvent<HTMLElement>) {
    if (event.button !== 0) {
      return;
    }

    void startDraggingCurrentWindow();
  }

  function onTitleDoubleClick(event: MouseEvent<HTMLElement>) {
    if (event.button !== 0) {
      return;
    }

    void resetTrayPopupSize();
  }

  function onResizeHandleMouseDown(event: MouseEvent<HTMLButtonElement>) {
    if (event.button !== 0) {
      return;
    }

    event.preventDefault();
    event.stopPropagation();
    void startResizingCurrentWindow();
  }

  return (
    <main className="tray-popup" data-testid="tray-popup">
      <header className="tray-popup__header">
        <div
          className="tray-popup__titlebar"
          data-testid="tray-popup-titlebar"
          title={t.tray.resetSize}
          onMouseDown={onTitleMouseDown}
          onDoubleClick={onTitleDoubleClick}
        >
          <div className="tray-popup__title-line">
            <h1>QuotaBarWin</h1>
            {primaryIssue ? (
              <span
                className={`status status--${primaryIssue.status} tray-popup__title-status`}
                title={providerIssues
                  .map((issue) => `${issue.provider.name}: ${t.tray.status[issue.status]}`)
                  .join("\n")}
              >
                {t.tray.healthSummary(
                  primaryIssue.provider.name,
                  primaryIssue.status,
                  providerIssues.length - 1
                )}
              </span>
            ) : null}
          </div>
          <p>
            {refreshedText
              ? t.tray.lastRefreshedAt(refreshedText)
              : t.tray.waitingForData}
          </p>
        </div>
        <div className="tray-popup__actions">
          <button className="button-compact button-secondary" type="button" onClick={() => void loadSnapshot()}>
            {isLoading ? t.tray.refreshingShort : t.tray.refresh}
          </button>
          <button
            className="button-compact button-ghost"
            type="button"
            onClick={() => void hideTrayPopup().catch(() => hideCurrentWindow())}
          >
            {t.tray.close}
          </button>
        </div>
      </header>

      <div className="tray-popup__content">
        {providers.length === 0 ? (
          <section className="tray-popup__empty">{t.tray.noProviders}</section>
        ) : null}

        {rows.length > 0 ? (
          <section className="tray-popup__windows" aria-label={t.tray.quotaWindowsLabel}>
            {rows.map(({ provider, window }) => {
              const providerStatus = calculateProviderStatus(provider, lowQuotaWarningThreshold);
              const quotaStatus = windowStatus(window, lowQuotaWarningThreshold);
              const displayedPercent = displayPercentForWindow(window, displayMode);
              const remainingPercent =
                window.remainingPercent ??
                (window.usedPercent !== null && window.usedPercent !== undefined ? 100 - window.usedPercent : null);
              const resetText = formatQuotaReset(window, new Date(), t);
              return (
                <article className="tray-popup__window" key={`${provider.id}-${window.id}`}>
                  <div className="tray-popup__window-title">
                    <strong>
                      {provider.name} - {window.label}
                    </strong>
                    <span>{formatDisplayValue(window, displayMode, t)}</span>
                  </div>
                  <ProgressBar
                    percent={displayedPercent}
                    opacityPercent={remainingPercent}
                    label={`${provider.name} ${window.label} ${displayMode}`}
                    tone={windowProgressTone(providerStatus, quotaStatus)}
                  />
                  {resetText ? <p>{resetText}</p> : null}
                </article>
              );
            })}
          </section>
        ) : null}
      </div>
      <button
        aria-label={t.tray.resize}
        className="tray-popup__resize-handle"
        data-testid="tray-popup-resize-handle"
        onMouseDown={onResizeHandleMouseDown}
        title={t.tray.resize}
        type="button"
      />
    </main>
  );
}
