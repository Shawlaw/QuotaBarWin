type HeaderProps = {
  isLoading: boolean;
  lastRefreshedAt: string | null;
  onRefresh: () => void;
  onOpenSettings: () => void;
};

export function Header({ isLoading, lastRefreshedAt, onRefresh, onOpenSettings }: HeaderProps) {
  return (
    <header className="app-header">
      <div>
        <h1>QuotaBarWin</h1>
        <p>{lastRefreshedAt ? `Last refresh ${new Date(lastRefreshedAt).toLocaleString()}` : "Not refreshed"}</p>
      </div>
      <div className="header-actions">
        <button type="button" className="button-secondary" onClick={onOpenSettings}>
          Settings
        </button>
        <button type="button" onClick={onRefresh} disabled={isLoading}>
          {isLoading ? "Refreshing" : "Refresh"}
        </button>
      </div>
    </header>
  );
}
