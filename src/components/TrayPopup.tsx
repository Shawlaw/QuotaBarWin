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

function progressTone(status: ProviderSnapshot["status"]): "normal" | "warning" | "error" {
  if (status === "error") {
    return "error";
  }

  if (status === "warning" || status === "stale") {
    return "warning";
  }

  return "normal";
}

function orderedProviders(providers: ProviderSnapshot[], config: AppConfig | null): ProviderSnapshot[] {
  if (!config) {
    return providers;
  }

  const providersById = new Map(providers.map((provider) => [provider.id, provider]));
  const orderedIds = new Set<string>();
  const configuredProviders = config.providers.flatMap((providerConfig) => {
    const provider = providersById.get(providerConfig.id);
    if (!provider) {
      return [];
    }

    orderedIds.add(provider.id);
    return [provider];
  });
  const remainingProviders = providers.filter((provider) => !orderedIds.has(provider.id));

  return [...configuredProviders, ...remainingProviders];
}

function orderedWindows(providers: ProviderSnapshot[]): WindowRow[] {
  return providers.flatMap((provider) => provider.windows.map((window) => ({ provider, window })));
}

export function TrayPopup() {
  const { t } = useI18n();
  const refreshInFlight = useRef(false);
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
    void listenForTrayPopupShown(() => void loadSnapshot()).then((cleanup) => {
      unlisten = cleanup;
    });

    return () => {
      unlisten?.();
    };
  }, [loadSnapshot]);

  useEffect(() => {
    function onKeyDown(event: KeyboardEvent) {
      if (event.key === "Escape") {
        void hideTrayPopup().catch(() => hideCurrentWindow());
      }
    }

    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, []);

  const providers = orderedProviders(snapshot?.providers ?? [], config);
  const lowQuotaWarningThreshold = config?.lowQuotaWarningThreshold ?? 20;
  const displayMode = config?.displayMode ?? "remaining";
  const rows = orderedWindows(providers);

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
          <h1>QuotaBarWin</h1>
          <p>{formatShortDateTime(snapshot?.refreshedAt) ?? t.tray.waitingForData}</p>
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
        ) : (
          <section className="tray-popup__providers" aria-label={t.tray.providerStatusLabel}>
            {providers.map((provider) => {
              const status = calculateProviderStatus(provider, lowQuotaWarningThreshold);
              return (
                <article className="tray-popup__provider" key={provider.id}>
                  <div>
                    <strong>{provider.name}</strong>
                    <span className={`status status--${status}`}>{t.tray.status[status]}</span>
                  </div>
                  {provider.error ? <p>{provider.error}</p> : null}
                </article>
              );
            })}
          </section>
        )}

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
                    <strong>{window.label}</strong>
                    <span>{formatDisplayValue(window, displayMode, t)}</span>
                  </div>
                  <ProgressBar
                    percent={displayedPercent}
                    opacityPercent={remainingPercent}
                    label={`${provider.name} ${window.label} ${displayMode}`}
                    tone={progressTone(status)}
                  />
                  <p>
                    {provider.name}
                    {resetText ? ` - ${resetText}` : ""}
                  </p>
                </article>
              );
            })}
          </section>
        ) : null}
      </div>
    </main>
  );
}
