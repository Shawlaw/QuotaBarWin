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
  logQuotaData?: boolean;
  language: AppLanguage;
  networkProxy?: ProxyConfig | null;
  trayPopupPosition?: TrayPopupPosition | null;
  trayPopupSize?: TrayPopupSize | null;
  appUpdate?: AppUpdateSettings;
  remoteProviderRegistry?: RemoteProviderRegistrySettings;
  providers: RemoteProviderConfig[];
};

export type AppUpdateSettings = {
  autoCheck: boolean;
};

export type RemoteProviderRegistrySettings = {
  registryUrl?: string | null;
  providerProxyUrl?: string | null;
  autoUpdate: boolean;
  sources?: RemoteProviderRegistrySource[];
};

export type RemoteProviderRegistrySource = {
  id: string;
  name: string;
  url: string;
  providerProxyUrl?: string | null;
  autoUpdate?: boolean;
  enabled: boolean;
};

export type RemoteProviderCatalogEntry = {
  id: string;
  displayName: string;
  version?: string | null;
  description?: string | null;
  providerUrl: string;
  checksum?: string | null;
  installed: boolean;
  installedCount?: number;
  error?: string | null;
};

export type RegistryMigrationFailure = {
  id: string;
  error: string;
};

export type RegistryMigrationResult = {
  migrated: string[];
  skipped: string[];
  failed: RegistryMigrationFailure[];
};

export type RemoteProviderManifestDefaultConfig = {
  name?: string | null;
  timeoutSeconds?: number | null;
  windowLabelOverrides?: Record<string, string>;
  visibleWindowIds?: string[];
  envVars?: Record<string, string>;
};

export type RemoteProviderParameter = {
  name: string;
  label?: string | null;
  kind?: "secret" | "string" | "number" | "select" | string | null;
  required?: boolean;
  defaultValue?: string | null;
  placeholder?: string | null;
  description?: string | null;
  options?: string[];
  helpUrl?: string | null;
  advanced?: boolean;
};

export type RemoteProviderManifest = {
  schemaVersion: number;
  id: string;
  displayName: string;
  version?: string | null;
  description?: string | null;
  minAppVersion?: string | null;
  runtime: string;
  entry: string;
  requiredEnvVars?: string[];
  output: string;
  permissions?: string[];
  defaultConfig?: RemoteProviderManifestDefaultConfig;
  parameters?: RemoteProviderParameter[];
  checksums?: {
    source?: string | null;
  };
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

export type ProxyTestErrorKind =
  | "noProxy"
  | "invalidTarget"
  | "invalidProxy"
  | "requestFailed"
  | "httpStatus";

export type ProxyTestResult = {
  success: boolean;
  statusCode?: number | null;
  elapsedMs: number;
  errorKind?: ProxyTestErrorKind | null;
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
  showInTray?: boolean;
  envVars?: Record<string, string>;
  setupState?: ProviderSetupState;
  setupLastTestedAt?: string | null;
};

export type ProviderSetupState = "pending" | "unverified" | "ready";

export type ProviderParameterSource =
  | "missing"
  | "managedLocalFile"
  | "secretFile"
  | "environment"
  | "externalFile"
  | "literal"
  | "default"
  | "advancedExpression";

export type ProviderSetupField = {
  name: string;
  label?: string | null;
  kind: "secret" | "string" | "number" | "select" | string;
  required: boolean;
  description?: string | null;
  placeholder?: string | null;
  options?: string[];
  helpUrl?: string | null;
  advanced?: boolean;
  value?: string | number | null;
  configured: boolean;
  source: ProviderParameterSource;
};

export type ProviderSetupDescriptor = {
  providerId: string;
  providerType: string;
  displayName: string;
  setupState: ProviderSetupState;
  fields: ProviderSetupField[];
  hasUnknownEnvVars: boolean;
  canAutoDetect: boolean;
};

export type SaveProviderSetupRequest = {
  providerId: string;
  displayName: string;
  values: Record<string, string | number | null>;
  secretUpdates: Record<string, string | null>;
};

export type ProviderSetupTestResult = {
  success: boolean;
  provider?: ProviderSnapshot | null;
  errorCategory?: ProviderErrorCategory | null;
};

export type ProviderErrorCategory =
  | "missingCredential"
  | "authentication"
  | "network"
  | "proxy"
  | "timeout"
  | "runtime"
  | "permission"
  | "providerOutput"
  | "unknown";
