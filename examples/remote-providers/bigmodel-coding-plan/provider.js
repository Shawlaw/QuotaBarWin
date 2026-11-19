function main(qb) {
  logStep(qb, "info", "provider.start", "BigModel provider started");
  const raw = fetchBigModelQuota(qb);
  if (raw?.success === false) {
    return { status: "error", error: raw.msg || "BigModel quota request failed", windows: [] };
  }
  const rawLimits = raw?.data?.limits;
  const limits = Array.isArray(rawLimits) ? rawLimits.filter(isUsableLimit) : [];
  if (limits.length === 0) {
    const error = "BigModel quota response contained no usable quota windows";
    logStep(qb, "warn", "snapshot.empty", error);
    return { status: "error", error, windows: [] };
  }
  logStep(qb, "info", "snapshot.ready", "BigModel snapshot ready", {
    status: "ok",
    windowCount: limits.length
  });
  return {
    status: "ok",
    updatedAt: qb.now(),
    windows: limits.map(limitToWindow).sort(compareWindows),
    metadata: {
      level: raw.data && raw.data.level || null,
      usageDetails: usageDetailsFrom(limits),
      rawCode: raw.code || null,
      rawMsg: raw.msg || null
    }
  };
}

function fetchBigModelQuota(qb) {
  const token = qb.env.get("BIGMODEL_API_KEY");
  logStep(qb, "info", "http.request.start", "Requesting BigModel quota", {
    endpoint: "open.bigmodel.cn/api/monitor/usage/quota/limit"
  });
  const response = qb.http.request("https://open.bigmodel.cn/api/monitor/usage/quota/limit", {
    headers: { Authorization: `Bearer ${token}` }
  });
  logStep(qb, response.ok ? "info" : "warn", "http.response", "BigModel quota response received", {
    status: response.status
  });
  if (!response.ok) throw new Error(`BigModel quota request failed with ${response.status}`);
  return JSON.parse(response.body);
}

function logStep(qb, level, stage, message, fields) {
  qb.log(Object.assign({ level, stage, message }, fields || {}));
}

function isUsableLimit(limit) {
  return typeof limit?.type === "string" && limit.type.trim() !== ""
    && numberOrNull(limit.unit) !== null && numberOrNull(limit.number) !== null
    && (numberOrNull(limit.percentage) !== null
      || (numberOrNull(limit.currentValue) !== null && numberOrNull(limit.usage) !== null));
}

function limitToWindow(limit) {
  const usedPercent = numberOrNull(limit.percentage);
  return {
    id: `${dash(limit.type)}-${limit.unit}-${limit.number}`.toLowerCase(),
    label: `${durationLabel(limit.unit, limit.number)} - ${titleCase(limit.type)}`,
    used: numberOrNull(limit.currentValue),
    limit: numberOrNull(limit.usage),
    unit: limit.type === "TOKENS_LIMIT" ? "tokens" : null,
    usedPercent,
    remainingPercent: usedPercent === null ? null : Math.max(0, 100 - usedPercent),
    resetAt: epochMsToIso(limit.nextResetTime),
    confidence: "exact"
  };
}

function usageDetailsFrom(limits) {
  const details = {};
  for (const limit of limits) {
    if (!Array.isArray(limit.usageDetails)) continue;
    for (const item of limit.usageDetails) {
      if (typeof item?.modelCode === "string") details[item.modelCode] = item.usage;
    }
  }
  return details;
}

function compareWindows(left, right) { return windowOrder(left.id) - windowOrder(right.id); }
function windowOrder(id) {
  return ({ "tokens-limit-3-5": 1, "tokens-limit-6-1": 2, "time-limit-5-1": 3 })[id] || 99;
}
function numberOrNull(value) {
  if (typeof value !== "number" && (typeof value !== "string" || value.trim() === "")) return null;
  const parsed = Number(value);
  return Number.isFinite(parsed) ? parsed : null;
}
function epochMsToIso(value) { const parsed = numberOrNull(value); return parsed === null ? null : new Date(parsed).toISOString(); }
function dash(value) { return String(value || "unknown").replace(/_/g, "-"); }
function titleCase(value) { return dash(value).toLowerCase().split("-").map((part) => part.charAt(0).toUpperCase() + part.slice(1)).join(" "); }
function durationLabel(unit, number) {
  const label = ({ 3: "hour", 5: "month", 6: "week" })[unit] || "window";
  return `${number} ${label}${Number(number) === 1 ? "" : "s"}`;
}
