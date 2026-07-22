import { useI18n } from "../i18n";
import { visibleAppVersion } from "../lib/appVersion";

type HeaderProps = {
  activeView: "overview" | "settings";
  appVersion: string;
  isLoading: boolean;
  onOpenOverview: () => void;
  onRefresh: () => void;
  onOpenSettings: () => void;
  onOpenGithub: () => void;
};

export function Header({
  activeView,
  appVersion,
  isLoading,
  onOpenOverview,
  onRefresh,
  onOpenSettings,
  onOpenGithub
}: HeaderProps) {
  const { t } = useI18n();
  const appVersionLabel = visibleAppVersion(appVersion);

  return (
    <header className="app-header">
      <div className="app-header__title">
        <h1>QuotaBarWin</h1>
        {appVersionLabel ? (
          <span className="app-header__app-version" title={t.app.versionTitle(appVersion)}>
            {appVersionLabel}
          </span>
        ) : null}
      </div>
      <div className="header-actions">
        <button
          type="button"
          className={activeView === "overview" ? "button-secondary" : undefined}
          onClick={onOpenOverview}
        >
          {t.header.overview}
        </button>
        <button
          type="button"
          className={activeView === "settings" ? "button-secondary" : undefined}
          onClick={onOpenSettings}
        >
          {t.header.settings}
        </button>
        <button
          type="button"
          className={`button-refresh${isLoading ? " button-refresh--busy" : ""}`}
          onClick={onRefresh}
          disabled={isLoading}
        >
          <span className="button-refresh__text">
            {isLoading ? t.header.refreshing : t.header.refresh}
          </span>
        </button>
        <button
          type="button"
          className="github-link"
          onClick={onOpenGithub}
        >
          GitHub
        </button>
      </div>
    </header>
  );
}
