export type ProviderStatus = "ok" | "warning" | "error" | "unknown";
export type ConfidenceLevel = "exact" | "estimated" | "unknown";
export type ProviderSource = "mock" | "command" | "native";

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

export type ProviderConfig = MockProviderConfig | CommandProviderConfig;

export type MockProviderConfig = {
  id: string;
  name: string;
  enabled: boolean;
  kind: "mock";
};

export type CommandProviderConfig = {
  id: string;
  name: string;
  enabled: boolean;
  kind: "command";
  command: CommandSpec;
  parser: ParserSpec;
  windowLabelOverrides?: Record<string, string>;
  visibleWindowIds?: string[];
};

export type CommandSpec = {
  executable: string;
  args: string[];
  cwd?: string | null;
  env?: Record<string, string>;
  timeoutMs: number;
};

export type ParserSpec =
  | { type: "app-snapshot" }
  | { type: "provider-snapshot" }
  | { type: "kimi-coding-usage-v1" }
  | { type: "bigmodel-quota-limit-json-v1" }
  | { type: "json-mapping"; mapping: Record<string, unknown> }
  | { type: "regex-blocks"; rules: Record<string, unknown>[] };

export type ProviderPreset = {
  id: string;
  displayName: string;
  description: string;
  providerConfigTemplate: ProviderConfig;
  requiredEnvVars?: string[];
  docs?: string | null;
};
