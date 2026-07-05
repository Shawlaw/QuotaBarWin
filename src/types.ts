export type ProviderStatus = "ok" | "warning" | "error" | "stale" | "unknown";
export type ConfidenceLevel = "exact" | "estimated" | "unknown";
export type ProviderSource = "mock" | "native" | "remote";
export type AppLanguage = "system" | "en" | "zh-CN";

export type QuotaWindow = {
  id: string;
  label: string;
  remaining?: number | null;
  used: number | null;
  limit: number | null;
  unit?: string | null;
  usedPercent: number | null;
  remainingPercent: number | null;
  warningRemaining?: number | null;
  resetAt: string | null;
  resetText?: string | null;
  confidence: ConfidenceLevel;
};

export type ProviderDiagnostics = {
  checkedAt: string;
  messages: string[];
  commandPath?: string | null;
  exitCode?: number | null;
  durationMs?: number | null;
  timedOut?: boolean | null;
  stderr?: string | null;
};

export type ProviderSnapshot = {
  id: string;
  name: string;
  status: ProviderStatus;
  source: ProviderSource;
  updatedAt: string | null;
  windows: QuotaWindow[];
  error?: string | null;
  diagnostics?: ProviderDiagnostics | null;
  metadata?: Record<string, unknown> | null;
};

export type AppSnapshot = {
  schemaVersion: 1;
  providers: ProviderSnapshot[];
  refreshedAt: string;
};

export type AppConfig = {
  schemaVersion: number;
  refreshIntervalSeconds: number;
  displayMode: "remaining" | "used";
  lowQuotaWarningThreshold: number;
  launchAtStartup?: boolean;
  logLevel?: "debug" | "info" | "warn" | "error" | string;
  logMaxBytes?: number;
  language: AppLanguage;
  networkProxy?: ProxyConfig | null;
  trayPopupPosition?: TrayPopupPosition | null;
  trayPopupSize?: TrayPopupSize | null;
  remoteProviderRegistry?: RemoteProviderRegistrySettings;
  providers: RemoteProviderConfig[];
};

export type RemoteProviderRegistrySettings = {
  registryUrl?: string | null;
  providerProxyUrl?: string | null;
  autoUpdate: boolean;
};

export type TrayPopupPosition = {
  x: number;
  y: number;
};

export type TrayPopupSize = {
  width: number;
  height: number;
};

export type ConfigStorageInfo = {
  mode: "app-data" | "portable" | string;
  configPath: string;
  configDir: string;
  appDataConfigPath: string;
  portableConfigPath: string;
  portableMarkerPath: string;
};

export type ProviderConfig = RemoteProviderConfig;

export type ProxyKind = "none" | "system" | "http" | "socks5";

export type ProxyConfig = {
  kind: ProxyKind;
  url?: string | null;
};

export type RemoteProviderConfig = {
  id: string;
  name: string;
  enabled: boolean;
  kind: "remote";
  version?: string | null;
  manifestUrl: string;
  sourceUrl: string;
  providerDir?: string | null;
  runtime: string;
  resolvedRuntime?: string | null;
  proxyUrl?: string | null;
  autoUpdate: boolean;
  updateIntervalSeconds: number;
  timeoutSeconds: number;
  trustedChecksum?: string | null;
  installedAt?: string | null;
  updatedAt?: string | null;
  lastCheckedAt?: string | null;
  windowLabelOverrides?: Record<string, string>;
  visibleWindowIds?: string[];
  envVars?: Record<string, string>;
};
