import type { AppUpdateInfo } from "../lib/api";
import { useI18n } from "../i18n";

type AppUpdateNoticeProps = {
  info: AppUpdateInfo | null;
  animate?: boolean;
  compact?: boolean;
  onOpenUpdate: () => void;
  onDismiss: () => void;
};

export function AppUpdateNotice({
  info,
  animate = false,
  compact = false,
  onOpenUpdate,
  onDismiss,
}: AppUpdateNoticeProps) {
  const { t } = useI18n();
  if (!info?.available || info.dismissed || !info.version) {
    return null;
  }

  return (
    <aside
      aria-live="polite"
      className={`app-update-notice${compact ? " app-update-notice--compact" : ""}${animate ? " app-update-notice--enter" : ""}`}
      data-testid={compact ? "tray-app-update-notice" : "app-update-notice"}
      role="status"
    >
      <span className="app-update-notice__message">{t.appUpdate.available(info.version)}</span>
      <div className="app-update-notice__actions">
        <button className="button-link" type="button" onClick={onOpenUpdate}>
          {t.appUpdate.openUpdate}
        </button>
        <button className="button-link" type="button" onClick={onDismiss}>
          {t.appUpdate.later}
        </button>
      </div>
    </aside>
  );
}
