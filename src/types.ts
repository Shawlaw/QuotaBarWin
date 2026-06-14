export type ProviderStatus = "ok" | "warning" | "error" | "stale" | "unknown";
export type ConfidenceLevel = "exact" | "estimated" | "unknown";
export type ProviderSource = "mock" | "native" | "remote";
export type AppLanguage = "system" | "en" | "zh-CN";

export type QuotaWindow = {
  id: string;
  label: string;
  used: number | null;
  limit: number | null;
  unit?: string | null;
  usedPercent: number | null;
  remainingPercent: number | null;
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
  language: AppLanguage;
  networkProxy?: ProxyConfig | null;
  providers: ProviderConfig[];
};

export type ConfigStorageInfo = {
  mode: "app-data" | "portable" | string;
  configPath: string;
  configDir: string;
  appDataConfigPath: string;
  portableConfigPath: string;
  portableMarkerPath: string;
};

export type ProviderConfig =
  | MockProviderConfig
  | CodexProviderConfig
  | RemoteProviderConfig;

export type MockProviderConfig = {
  id: string;
  name: string;
  enabled: boolean;
  kind: "mock";
};

export type CodexProviderConfig = {
  id: string;
  name: string;
  enabled: boolean;
  kind: "codex";
  authToken: string;
  accountId?: string | null;
  proxyUrl?: string | null;
  timeoutMs: number;
  windowLabelOverrides?: Record<string, string>;
  visibleWindowIds?: string[];
};

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
  manifestUrl: string;
  sourceUrl: string;
  providerDir?: string | null;
  runtime: string;
  resolvedRuntime?: string | null;
  proxyUrl?: string | null;
  autoUpdate: boolean;
  updateIntervalSeconds: number;
  trustedChecksum?: string | null;
  windowLabelOverrides?: Record<string, string>;
  visibleWindowIds?: string[];
  envVars?: Record<string, string>;
};

export type ProviderPreset = {
  id: string;
  displayName: string;
  description: string;
  providerConfigTemplate: ProviderConfig;
  requiredEnvVars?: string[];
  docs?: string | null;
};
