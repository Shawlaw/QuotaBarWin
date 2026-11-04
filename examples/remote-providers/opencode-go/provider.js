const OPENCODE_GO_USAGE_URL = "https://opencode.ai/zen/go/v1/usage";

function main(qb) {
  logStep(qb, "info", "provider.start", "OpenCode Go provider started");
  const apiKey = qb.env.get("OPENCODE_GO_API_KEY").trim();
  if (!apiKey) {
    throw new Error("OpenCode Go API key is required. Create one in the OpenCode Console and paste it into the provider setup form.");
  }
  logStep(qb, "info", "auth.loaded", "OpenCode Go API key loaded");

  const raw = fetchOpenCodeGoUsage(qb, apiKey);
  const usage = usagePayload(raw);
  if (!usage) {
    throw new Error("OpenCode Go did not return subscription usage. Make sure the API key belongs to an account with an active Go subscription.");
  }

  const nowMs = Date.now();
  const windows = [];
  pushWindow(windows, "5h", "5h", usage, ["rolling", "rollingUsage"], nowMs);
  pushWindow(windows, "weekly", "Weekly limit", usage, ["weekly", "weeklyUsage"], nowMs);
  pushWindow(windows, "monthly", "Monthly limit", usage, ["monthly", "monthlyUsage"], nowMs);

  logStep(qb, windows.length > 0 ? "info" : "warn", "snapshot.ready", "OpenCode Go snapshot ready", {
    status: windows.length > 0 ? "ok" : "warning",
    windowCount: windows.length
  });
  return {
    status: windows.length > 0 ? "ok" : "warning",
    updatedAt: qb.now(),
    windows,
    metadata: metadataFrom(raw, usage)
  };
}

function fetchOpenCodeGoUsage(qb, apiKey) {
  logStep(qb, "info", "usage.request.start", "OpenCode Go usage request started");
  const response = qb.http.request(OPENCODE_GO_USAGE_URL, {
    headers: {
      Authorization: `Bearer ${apiKey}`,
      Accept: "application/json",
      "User-Agent": "QuotaBarWin/1.7"
    }
  });
  logStep(qb, response.ok ? "info" : "warn", "usage.response", "OpenCode Go usage response received", {
    status: response.status,
    bodyBytes: response.body.length
  });
  if (response.status === 401) {
    throw new Error("OpenCode Go rejected the API key (HTTP 401). Check the key or create a new one in the OpenCode Console.");
  }
  if (response.status === 403) {
    throw new Error("OpenCode Go denied access to usage (HTTP 403). The key may lack the Usage permission or the account may not have a Go subscription.");
  }
  if (!response.ok) {
    throw new Error(`OpenCode Go usage API returned ${response.status}: ${response.body.slice(0, 160)}`);
  }
  return JSON.parse(response.body);
}

function usagePayload(raw) {
  if (!raw || typeof raw !== "object") return null;
  if (raw.usage && typeof raw.usage === "object") return raw.usage;
  if (raw.data && typeof raw.data === "object" && raw.data.usage && typeof raw.data.usage === "object") {
    return raw.data.usage;
  }
  return null;
}

function pushWindow(windows, id, label, usage, names, nowMs) {
  const source = firstWindow(usage, names);
  if (!source) return;
  const percent = firstNumber(source, ["percent", "usagePercent"]);
  if (percent === null) return;
  const usedPercent = clamp(percent, 0, 100);
  windows.push({
    id,
    label,
    used: null,
    limit: null,
    unit: "percent",
    usedPercent,
    remainingPercent: Math.max(0, 100 - usedPercent),
    resetAt: resetAtFrom(source, nowMs),
    resetText: null,
    confidence: "exact"
  });
}

function firstWindow(usage, names) {
  for (const name of names) {
    const value = usage[name];
    if (value && typeof value === "object") return value;
  }
  if (Array.isArray(usage.windows)) {
    for (const entry of usage.windows) {
      if (!entry || typeof entry !== "object") continue;
      const key = String(entry.name || entry.id || "").toLowerCase();
      if (names.indexOf(key) !== -1) return entry;
    }
  }
  return null;
}

function resetAtFrom(source, nowMs) {
  const resetsAt = source.resetsAt;
  if (typeof resetsAt === "string" && resetsAt.trim() !== "") {
    const parsed = Date.parse(resetsAt);
    if (Number.isFinite(parsed)) return new Date(parsed).toISOString();
  }
  const resetInSec = firstNumber(source, ["resetInSec", "resetAfterSeconds"]);
  if (resetInSec !== null && resetInSec >= 0) {
    return new Date(nowMs + resetInSec * 1000).toISOString();
  }
  return null;
}

function metadataFrom(raw, usage) {
  const billing = raw && raw.billing && typeof raw.billing === "object" ? raw.billing : {};
  const topLevelBalance = booleanOrNull(raw && raw.useBalance);
  return {
    plan: stringOrNull(raw && raw.plan) || stringOrNull(billing.subscriptionPlan) || null,
    useBalance: topLevelBalance !== null ? topLevelBalance : booleanOrNull(usage && usage.useBalance)
  };
}

function firstNumber(source, keys) {
  for (const key of keys) {
    const value = numberOrNull(source[key]);
    if (value !== null) return value;
  }
  return null;
}
function numberOrNull(value) {
  if (value === null || value === undefined || value === "") return null;
  const parsed = Number(value);
  return Number.isFinite(parsed) ? parsed : null;
}
function booleanOrNull(value) {
  return typeof value === "boolean" ? value : null;
}
function stringOrNull(value) {
  return typeof value === "string" && value.trim() !== "" ? value.trim() : null;
}
function clamp(value, min, max) {
  return Math.min(Math.max(value, min), max);
}
function logStep(qb, level, stage, message, fields) {
  qb.log(Object.assign({ level, stage, message }, fields || {}));
}
