function main(qb) {
  logStep(qb, "info", "provider.start", "Kimi provider started");
  const raw = fetchKimiUsage(qb);
  const windows = [];
  const fiveHour = (raw.limits || []).find(
    (limit) => Number(limit && limit.window && limit.window.duration) === 300
      && limit.window.timeUnit === "TIME_UNIT_MINUTE"
  );
  if (fiveHour && fiveHour.detail) {
    windows.push(windowFromUsage("300-minute", "5h", fiveHour.detail, { unknownMeansFull: true }));
  }
  if (raw.usage) {
    windows.push(windowFromUsage("usage", "Weekly limit", raw.usage));
  }
  if (raw.totalQuota) {
    const limit = numberOrNull(raw.totalQuota.limit);
    const remaining = numberOrNull(raw.totalQuota.remaining);
    windows.push({
      id: "total-quota",
      label: "Total quota",
      used: limit !== null && remaining !== null ? Math.max(0, limit - remaining) : null,
      limit,
      unit: null,
      resetAt: null,
      confidence: "exact"
    });
  }
  logStep(qb, "info", "snapshot.ready", "Kimi snapshot ready", { windowCount: windows.length });
  return {
    status: "ok",
    updatedAt: qb.now(),
    windows,
    metadata: {
      region: raw.user && raw.user.region || null,
      membershipLevel: raw.user && raw.user.membership && raw.user.membership.level || null,
      authMethod: raw.authentication && raw.authentication.method || null,
      authScope: raw.authentication && raw.authentication.scope || null,
      subType: raw.subType || null,
      parallelLimit: numberOrNull(raw.parallel && raw.parallel.limit)
    }
  };
}

function fetchKimiUsage(qb) {
  const token = qb.env.get("KIMI_API_KEY");
  logStep(qb, "info", "http.request.start", "Requesting Kimi usage", {
    endpoint: "api.kimi.com/coding/v1/usages"
  });
  const response = qb.http.request("https://api.kimi.com/coding/v1/usages", {
    headers: { Authorization: `Bearer ${token}` }
  });
  logStep(qb, response.ok ? "info" : "warn", "http.response", "Kimi usage response received", {
    status: response.status
  });
  if (!response.ok) {
    throw new Error(`Kimi usage request failed with ${response.status}`);
  }
  return JSON.parse(response.body);
}

function logStep(qb, level, stage, message, fields) {
  qb.log(Object.assign({ level, stage, message }, fields || {}));
}

function windowFromUsage(id, label, usage, options) {
  const used = numberOrNull(usage.used);
  const remaining = numberOrNull(usage.remaining);
  const limit = numberOrNull(usage.limit);
  const percentages = percentagesFromUsage({ used, remaining, limit }, options || {});
  return {
    id,
    label,
    remaining,
    used,
    limit,
    unit: null,
    usedPercent: percentages.usedPercent,
    remainingPercent: percentages.remainingPercent,
    resetAt: usage.resetTime || null,
    confidence: percentages.estimated ? "estimated" : "exact"
  };
}

function percentagesFromUsage(values, options) {
  if (values.limit !== null && values.limit > 0 && values.used !== null) {
    const usedPercent = clampPercent(values.used / values.limit * 100);
    return { usedPercent, remainingPercent: clampPercent(100 - usedPercent), estimated: false };
  }
  if (values.limit !== null && values.limit > 0 && values.remaining !== null) {
    const remainingPercent = clampPercent(values.remaining / values.limit * 100);
    return { usedPercent: clampPercent(100 - remainingPercent), remainingPercent, estimated: false };
  }
  if (options.unknownMeansFull && values.used === null && values.remaining === null && values.limit === null) {
    return { usedPercent: 0, remainingPercent: 100, estimated: true };
  }
  return { usedPercent: null, remainingPercent: null, estimated: false };
}

function clampPercent(value) {
  return Math.min(100, Math.max(0, value));
}

function numberOrNull(value) {
  if (value === null || value === undefined || value === "") return null;
  const parsed = Number(value);
  return Number.isFinite(parsed) ? parsed : null;
}
