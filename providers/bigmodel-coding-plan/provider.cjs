const fs = require("node:fs/promises");

(async function main() {
  const fixturePath = process.env.QUOTABARWIN_BIGMODEL_FIXTURE;
  const raw = fixturePath ? await readJson(fixturePath) : await fetchBigModelQuota();
  const limits = raw.data?.limits ?? [];

  console.log(
    JSON.stringify({
      status: raw.success === false ? "error" : "ok",
      updatedAt: new Date().toISOString(),
      windows: limits.map(limitToWindow).sort(compareWindows),
      error: raw.success === false ? raw.msg ?? "BigModel quota request failed" : null,
      metadata: {
        level: raw.data?.level ?? null,
        usageDetails: usageDetailsFrom(limits),
        rawCode: raw.code ?? null,
        rawMsg: raw.msg ?? null
      }
    })
  );
})().catch((error) => {
  console.error(error instanceof Error ? error.message : String(error));
  process.exit(1);
});

async function fetchBigModelQuota() {
  const token = process.env.BIGMODEL_API_KEY;
  if (!token) {
    throw new Error("BIGMODEL_API_KEY is required");
  }

  const response = await fetch("https://open.bigmodel.cn/api/monitor/usage/quota/limit", {
    headers: { Authorization: `Bearer ${token}` }
  });
  if (!response.ok) {
    throw new Error(`BigModel quota request failed with ${response.status}`);
  }
  return response.json();
}

async function readJson(filePath) {
  return JSON.parse(await fs.readFile(filePath, "utf8"));
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
    for (const item of limit.usageDetails ?? []) {
      details[item.modelCode] = item.usage;
    }
  }
  return details;
}

function compareWindows(left, right) {
  return windowOrder(left.id) - windowOrder(right.id);
}

function windowOrder(id) {
  const order = {
    "tokens-limit-3-5": 1,
    "tokens-limit-6-1": 2,
    "time-limit-5-1": 3
  };
  return order[id] ?? 99;
}

function numberOrNull(value) {
  if (value === null || value === undefined || value === "") {
    return null;
  }
  const parsed = Number(value);
  return Number.isFinite(parsed) ? parsed : null;
}

function epochMsToIso(value) {
  const parsed = numberOrNull(value);
  return parsed === null ? null : new Date(parsed).toISOString();
}

function dash(value) {
  return String(value ?? "unknown").replace(/_/g, "-");
}

function titleCase(value) {
  return dash(value)
    .toLowerCase()
    .split("-")
    .map((part) => part.charAt(0).toUpperCase() + part.slice(1))
    .join(" ");
}

function durationLabel(unit, number) {
  const units = {
    3: "hour",
    5: "month",
    6: "week"
  };
  const label = units[unit] ?? "window";
  return `${number} ${label}${Number(number) === 1 ? "" : "s"}`;
}
