import { useI18n } from "../i18n";

type HeaderProps = {
  activeView: "overview" | "settings";
  appVersion: string;
  isLoading: boolean;
  onOpenOverview: () => void;
  onRefresh: () => void;
  onOpenSettings: () => void;
};

export function Header({
  activeView,
  appVersion,
  isLoading,
  onOpenOverview,
  onRefresh,
  onOpenSettings
}: HeaderProps) {
  const { t } = useI18n();

  return (
    <header className="app-header">
      <div>
        <h1 title={t.app.versionTitle(appVersion)}>QuotaBarWin</h1>
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
        <a
          className="github-link"
          href="https://github.com/Shawlaw/QuotaBarWin"
          target="_blank"
          rel="noreferrer"
        >
          GitHub
        </a>
      </div>
    </header>
  );
}
