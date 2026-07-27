const CODEX_USAGE_URL = "https://chatgpt.com/backend-api/wham/usage";
const CODEX_SESSION_WINDOW_SECONDS = 5 * 60 * 60;
const CODEX_WEEKLY_WINDOW_SECONDS = 7 * 24 * 60 * 60;

function main(qb) {
  logStep(qb, "info", "provider.start", "Codex provider started");
  const auth = readCodexAuth(qb);
  const explicitToken = qb.env.getOptional("CODEX_ACCESS_TOKEN");
  const explicitAccountId = qb.env.getOptional("CODEX_ACCOUNT_ID");
  const token = explicitToken || auth.accessToken;
  const accountId = explicitAccountId || auth.accountId || "";
  logStep(qb, "info", "auth.loaded", "Codex auth loaded", {
    tokenSource: explicitToken ? "env" : auth.accessToken ? "auth-file" : "missing",
    accountIdPresent: Boolean(accountId)
  });
  if (!token) {
    throw new Error("Codex sign-in is required. Sign in with Codex or configure CODEX_ACCESS_TOKEN.");
  }

  const raw = fetchCodexUsage(qb, token.trim(), accountId.trim());
  const normalized = normalizeRateLimitWindows(raw.rate_limit);
  const windows = [];
  if (normalized.session) windows.push(windowFromUsage("5h", "5h", normalized.session));
  if (normalized.weekly) windows.push(windowFromUsage("weekly", "Weekly limit", normalized.weekly));
  logStep(qb, "info", "snapshot.ready", "Codex snapshot ready", {
    status: windows.length > 0 ? "ok" : "warning",
    windowCount: windows.length,
    planType: raw.plan_type || null
  });
  return {
    status: windows.length > 0 ? "ok" : "warning",
    updatedAt: qb.now(),
    windows,
    metadata: { planType: raw.plan_type || null, credits: raw.credits || null }
  };
}

function readCodexAuth(qb) {
  const override = qb.env.getOptional("CODEX_AUTH_FILE");
  const authPath = override || "~/.codex/auth.json";
  try {
    const auth = JSON.parse(qb.fs.readText(authPath));
    logStep(qb, "info", "auth.file.read", "Codex auth file parsed", {
      hasAccessToken: Boolean(auth && auth.tokens && auth.tokens.access_token),
      hasAccountId: Boolean(auth && auth.tokens && auth.tokens.account_id)
    });
    return {
      accessToken: stringOrEmpty(auth && auth.tokens && auth.tokens.access_token),
      accountId: stringOrEmpty(auth && auth.tokens && auth.tokens.account_id)
    };
  } catch (_) {
    logStep(qb, "warn", "auth.file.read", "Codex auth file could not be read");
    return { accessToken: "", accountId: "" };
  }
}

function fetchCodexUsage(qb, token, accountId) {
  const headers = { Authorization: `Bearer ${token}`, "User-Agent": "QuotaBarWin/1.1" };
  if (accountId) headers["ChatGPT-Account-Id"] = accountId;
  logStep(qb, "info", "usage.request.start", "Codex usage request started");
  const response = qb.http.request(CODEX_USAGE_URL, { headers });
  logStep(qb, response.ok ? "info" : "warn", "usage.response", "Codex usage response received", {
    status: response.status,
    bodyBytes: response.body.length
  });
  if (!response.ok) {
    throw new Error(`Codex usage API returned ${response.status}: ${response.body.slice(0, 160)}`);
  }
  const parsed = JSON.parse(response.body);
  logStep(qb, "info", "usage.parse.done", "Codex usage response parsed");
  return parsed;
}

function windowFromUsage(id, label, usage) {
  const usedPercent = numberOrNull(usage.used_percent);
  return {
    id,
    label,
    used: null,
    limit: null,
    unit: "percent",
    usedPercent,
    remainingPercent: usedPercent === null ? null : Math.max(0, 100 - usedPercent),
    resetAt: resetIsoFromWindow(usage),
    resetText: null,
    confidence: "exact"
  };
}

function normalizeRateLimitWindows(rateLimit) {
  const primary = usageWindowOrNull(rateLimit && rateLimit.primary_window);
  const secondary = usageWindowOrNull(rateLimit && rateLimit.secondary_window);
  const candidates = [primary, secondary].filter(Boolean);
  let session = candidates.find((window) => rateWindowRole(window) === "session") || null;
  let weekly = candidates.find((window) => rateWindowRole(window) === "weekly") || null;
  if (!session) session = [primary, secondary].find((window) => window && window !== weekly) || null;
  if (!weekly) weekly = [secondary, primary].find((window) => window && window !== session) || null;
  return { session, weekly };
}

function usageWindowOrNull(window) { return window && typeof window.used_percent === "number" ? window : null; }
function rateWindowRole(window) {
  const seconds = numberOrNull(window && window.limit_window_seconds);
  if (seconds === CODEX_SESSION_WINDOW_SECONDS) return "session";
  if (seconds === CODEX_WEEKLY_WINDOW_SECONDS) return "weekly";
  return "unknown";
}
function resetIsoFromWindow(usage) {
  if (typeof usage.reset_at === "number" && usage.reset_at > 0) return new Date(usage.reset_at * 1000).toISOString();
  if (typeof usage.reset_after_seconds === "number" && usage.reset_after_seconds > 0) return new Date(Date.now() + usage.reset_after_seconds * 1000).toISOString();
  return null;
}
function numberOrNull(value) {
  if (value === null || value === undefined || value === "") return null;
  const parsed = Number(value);
  return Number.isFinite(parsed) ? parsed : null;
}
function stringOrEmpty(value) { return typeof value === "string" ? value.trim() : ""; }
function logStep(qb, level, stage, message, fields) { qb.log(Object.assign({ level, stage, message }, fields || {})); }
