import { useCallback, useEffect, useRef, useState, type MouseEvent } from "react";
import {
  getCachedSnapshot,
  getConfig,
  hideCurrentWindow,
  hideTrayPopup,
  listenForTrayPopupShown,
  refreshSnapshot,
  startDraggingCurrentWindow
} from "../lib/api";
import {
  calculateProviderStatus,
  displayPercentForWindow,
  formatDisplayValue,
  formatQuotaReset,
  formatShortDateTime
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

const PRESENTATION_REFRESH_DEDUPE_MS = 750;

function progressTone(status: ProviderSnapshot["status"]): "normal" | "warning" | "error" {
  if (status === "error") {
    return "error";
  }

  if (status === "warning" || status === "stale") {
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
  const presentationRefreshSuppressedUntil = useRef(0);
  const [snapshot, setSnapshot] = useState<AppSnapshot | null>(null);
  const [config, setConfig] = useState<AppConfig | null>(null);
  const [isLoading, setIsLoading] = useState(false);

  const loadSnapshot = useCallback(async () => {
    if (refreshInFlight.current) {
      return;
    }

    refreshInFlight.current = true;
    setIsLoading(true);
    try {
      setSnapshot(await refreshSnapshot());
    } catch {
      setSnapshot(await getCachedSnapshot());
    } finally {
      refreshInFlight.current = false;
      setIsLoading(false);
    }
  }, []);

  const refreshForPresentation = useCallback(() => {
    const now = Date.now();
    if (now < presentationRefreshSuppressedUntil.current) {
      return;
    }

    presentationRefreshSuppressedUntil.current = now + PRESENTATION_REFRESH_DEDUPE_MS;
    void loadSnapshot();
  }, [loadSnapshot]);

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
    });

    return () => {
      unlisten?.();
    };
  }, [refreshForPresentation]);

  useEffect(() => {
    function onInitialFocus() {
      window.removeEventListener("focus", onInitialFocus);
      refreshForPresentation();
    }

    window.addEventListener("focus", onInitialFocus);
    return () => window.removeEventListener("focus", onInitialFocus);
  }, [refreshForPresentation]);

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

  return (
    <main className="tray-popup" data-testid="tray-popup">
      <header className="tray-popup__header">
        <div
          className="tray-popup__titlebar"
          data-testid="tray-popup-titlebar"
          onMouseDown={onTitleMouseDown}
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
              const status = calculateProviderStatus(provider, lowQuotaWarningThreshold);
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
                    tone={progressTone(status)}
                  />
                  {resetText ? <p>{resetText}</p> : null}
                </article>
              );
            })}
          </section>
        ) : null}
      </div>
    </main>
  );
}
