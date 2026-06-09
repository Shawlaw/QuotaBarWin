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
  return (
    <header className="app-header">
      <div>
        <h1>QuotaBarWin V{appVersion}</h1>
      </div>
      <div className="header-actions">
        <button
          type="button"
          className={activeView === "overview" ? undefined : "button-secondary"}
          onClick={onOpenOverview}
        >
          Overview
        </button>
        <button
          type="button"
          className={activeView === "settings" ? undefined : "button-secondary"}
          onClick={onOpenSettings}
        >
          Settings
        </button>
        <button
          type="button"
          className={`button-refresh${isLoading ? " button-refresh--busy" : ""}`}
          onClick={onRefresh}
          disabled={isLoading}
        >
          <span className="button-refresh__text">
            {isLoading ? "Refreshing" : "Refresh"}
          </span>
        </button>
      </div>
    </header>
  );
}
