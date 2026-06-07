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
