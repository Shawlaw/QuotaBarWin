type HeaderProps = {
  isLoading: boolean;
  onRefresh: () => void;
};

export function Header({ isLoading, onRefresh }: HeaderProps) {
  return (
    <header className="app-header">
      <div>
        <h1>QuotaBarWin</h1>
        <p>Provider runtime technical spike</p>
      </div>
      <button type="button" onClick={onRefresh} disabled={isLoading}>
        {isLoading ? "Refreshing" : "Refresh"}
      </button>
    </header>
  );
}
