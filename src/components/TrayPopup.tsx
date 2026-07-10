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
  setTrayPopupAutoHeight,
  showMainWindow,
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
import type { AppConfig, AppSnapshot, ProviderSnapshot } from "../types";
import { useI18n } from "../i18n";
import { ProgressBar } from "./ProgressBar";

const DEFAULT_TRAY_POPUP_WIDTH = 380;
const DEFAULT_TRAY_POPUP_HEIGHT = 520;
const TRAY_POPUP_AUTO_MIN_HEIGHT = 220;
const TRAY_POPUP_AUTO_MAX_HEIGHT = 640;

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

function numericCssValue(value: string): number {
  const parsed = Number.parseFloat(value);
  return Number.isFinite(parsed) ? parsed : 0;
}

function isDefaultTrayPopupSize(size: AppConfig["trayPopupSize"]): boolean {
  if (!size) {
    return false;
  }

  return (
    Math.abs(size.width - DEFAULT_TRAY_POPUP_WIDTH) < 1 &&
    Math.abs(size.height - DEFAULT_TRAY_POPUP_HEIGHT) < 1
  );
}

function shouldAutoSizeTrayPopup(config: AppConfig | null): boolean {
  if (!config) {
    return false;
  }

  if (!config.trayPopupSize) {
    return true;
  }

  return isDefaultTrayPopupSize(config.trayPopupSize);
}

export function TrayPopup() {
  const { t } = useI18n();
  const popupRef = useRef<HTMLElement | null>(null);
  const headerRef = useRef<HTMLElement | null>(null);
  const contentRef = useRef<HTMLDivElement | null>(null);
  const refreshInFlight = useRef(false);
  const lastHandledPresentationId = useRef(0);
  const lastRequestedAutoHeight = useRef<number | null>(null);
  const [snapshot, setSnapshot] = useState<AppSnapshot | null>(null);
  const [config, setConfig] = useState<AppConfig | null>(null);
  const [isLoading, setIsLoading] = useState(false);
  const [hasSessionManualSize, setHasSessionManualSize] = useState(false);

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

    void getCachedSnapshot()
      .then((cached) => {
        if (isMounted) {
          setSnapshot(cached);
        }
      })
      .catch(() => undefined);

    async function initialize() {
      const loadedConfig = await getConfig();
      if (!isMounted) {
        return;
      }

      setConfig(loadedConfig);
    }

    void initialize();

    return () => {
      isMounted = false;
    };
  }, []);

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    void listenForTrayPopupShown(refreshForPresentation).then((cleanup) => {
      unlisten = cleanup;
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

  const hasSnapshot = snapshot !== null;
  const providers = snapshot?.providers ?? [];
  const lowQuotaWarningThreshold = config?.lowQuotaWarningThreshold ?? 20;
  const displayMode = config?.displayMode ?? "remaining";
  const providersWithWindows = providers.filter((provider) => provider.windows.length > 0);
  const providerIssues = orderedProviderIssues(providers, lowQuotaWarningThreshold);
  const primaryIssue = providerIssues[0] ?? null;
  const refreshedText = formatShortDateTime(snapshot?.refreshedAt);

  useEffect(() => {
    if (hasSessionManualSize || !shouldAutoSizeTrayPopup(config)) {
      return;
    }

    let frameId = 0;
    let disposed = false;

    function requestAutoHeight() {
      if (disposed) {
        return;
      }

      window.cancelAnimationFrame(frameId);
      frameId = window.requestAnimationFrame(() => {
        const popup = popupRef.current;
        const header = headerRef.current;
        const content = contentRef.current;
        if (!popup || !header || !content) {
          return;
        }

        const popupStyle = window.getComputedStyle(popup);
        const verticalPadding =
          numericCssValue(popupStyle.paddingTop) + numericCssValue(popupStyle.paddingBottom);
        const rowGap = numericCssValue(popupStyle.rowGap || popupStyle.gap);
        const contentHeight = content.scrollHeight;
        const headerHeight = header.getBoundingClientRect().height;
        const desiredHeight = Math.ceil(
          verticalPadding + headerHeight + rowGap + contentHeight
        );
        const nextHeight = Math.max(
          TRAY_POPUP_AUTO_MIN_HEIGHT,
          Math.min(TRAY_POPUP_AUTO_MAX_HEIGHT, desiredHeight)
        );

        if (
          lastRequestedAutoHeight.current !== null &&
          Math.abs(lastRequestedAutoHeight.current - nextHeight) < 1
        ) {
          return;
        }

        lastRequestedAutoHeight.current = nextHeight;
        void setTrayPopupAutoHeight(nextHeight);
      });
    }

    requestAutoHeight();
    const resizeObserver =
      typeof ResizeObserver === "undefined" ? null : new ResizeObserver(requestAutoHeight);
    if (resizeObserver) {
      if (headerRef.current) {
        resizeObserver.observe(headerRef.current);
      }
      if (contentRef.current) {
        resizeObserver.observe(contentRef.current);
      }
      const contentChild = contentRef.current?.firstElementChild;
      if (contentChild instanceof HTMLElement) {
        resizeObserver.observe(contentChild);
      }
    }
    window.addEventListener("resize", requestAutoHeight);

    return () => {
      disposed = true;
      window.cancelAnimationFrame(frameId);
      resizeObserver?.disconnect();
      window.removeEventListener("resize", requestAutoHeight);
    };
  }, [config, displayMode, hasSessionManualSize, isLoading, snapshot, t]);

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

    void resetTrayPopupSize()
      .then(async () => {
        lastRequestedAutoHeight.current = null;
        setHasSessionManualSize(false);
        setConfig(await getConfig());
      })
      .catch(() => undefined);
  }

  function onResizeHandleMouseDown(event: MouseEvent<HTMLButtonElement>) {
    if (event.button !== 0) {
      return;
    }

    event.preventDefault();
    event.stopPropagation();
    setHasSessionManualSize(true);
    void startResizingCurrentWindow();
  }

  async function openMainWindow() {
    try {
      await showMainWindow();
      await hideTrayPopup().catch(() => hideCurrentWindow());
    } catch {
      // Keep the popup open if the main window could not be shown.
    }
  }

  return (
    <main className="tray-popup" data-testid="tray-popup" ref={popupRef}>
      <header className="tray-popup__header" ref={headerRef}>
        <div
          className="tray-popup__titlebar"
          data-testid="tray-popup-titlebar"
          title={t.tray.resetSize}
          onMouseDown={onTitleMouseDown}
          onDoubleClick={onTitleDoubleClick}
        >
          <div className="tray-popup__top-row">
            <h1>QuotaBarWin</h1>
            <div
              className="tray-popup__actions"
              onMouseDown={(event) => event.stopPropagation()}
              onDoubleClick={(event) => event.stopPropagation()}
            >
              <button
                aria-label={t.tray.openMainWindow}
                className="button-compact button-secondary tray-popup__action-button"
                type="button"
                onClick={() => void openMainWindow()}
                data-testid="tray-popup-open-main"
                title={t.tray.openMainWindow}
              >
                {t.tray.openMainWindowShort}
              </button>
              <button
                aria-label={t.tray.refresh}
                className="button-compact button-secondary tray-popup__action-button"
                type="button"
                onClick={() => void loadSnapshot()}
                data-testid="tray-popup-refresh"
                title={t.tray.refresh}
              >
                {isLoading ? t.tray.refreshingShort : t.tray.refresh}
              </button>
              <button
                aria-label={t.tray.close}
                className="button-compact button-ghost tray-popup__action-button"
                type="button"
                onClick={() => void hideTrayPopup().catch(() => hideCurrentWindow())}
                data-testid="tray-popup-close"
                title={t.tray.close}
              >
                {t.tray.close}
              </button>
            </div>
          </div>
          <div className="tray-popup__meta-row">
            <p>
              {refreshedText
                ? t.tray.lastRefreshedAt(refreshedText)
                : t.tray.waitingForData}
            </p>
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
        </div>
      </header>

      <div className="tray-popup__content" ref={contentRef}>
        {hasSnapshot && providers.length === 0 ? (
          <section className="tray-popup__empty">{t.tray.noProviders}</section>
        ) : null}

        {providersWithWindows.length > 0 ? (
          <section className="tray-popup__provider-groups" aria-label={t.tray.quotaWindowsLabel}>
            {providersWithWindows.map((provider) => {
              const providerStatus = calculateProviderStatus(provider, lowQuotaWarningThreshold);
              return (
                <article className="tray-popup__provider-group" key={provider.id}>
                  <header className="tray-popup__provider-header">
                    <strong>{provider.name}</strong>
                    {providerStatus !== "ok" ? (
                      <span className={`status status--${providerStatus}`}>
                        {t.tray.status[providerStatus]}
                      </span>
                    ) : null}
                  </header>
                  <div className="tray-popup__provider-windows">
                    {provider.windows.map((window) => {
                      const quotaStatus = windowStatus(window, lowQuotaWarningThreshold);
                      const displayedPercent = displayPercentForWindow(window, displayMode);
                      const remainingPercent =
                        window.remainingPercent ??
                        (window.usedPercent !== null && window.usedPercent !== undefined
                          ? 100 - window.usedPercent
                          : null);
                      const resetText = formatQuotaReset(window, new Date(), t);
                      return (
                        <section className="tray-popup__window" key={window.id}>
                          <div className="tray-popup__window-title">
                            <strong>{window.label}</strong>
                            <span>{formatDisplayValue(window, displayMode, t)}</span>
                          </div>
                          <ProgressBar
                            percent={displayedPercent}
                            opacityPercent={remainingPercent}
                            label={`${provider.name} ${window.label} ${displayMode}`}
                            tone={windowProgressTone(providerStatus, quotaStatus)}
                          />
                          {resetText ? <p>{resetText}</p> : null}
                        </section>
                      );
                    })}
                  </div>
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
