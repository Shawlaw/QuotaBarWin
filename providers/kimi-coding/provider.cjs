const fs = require("node:fs/promises");

(async function main() {
  const fixturePath = process.env.QUOTABARWIN_KIMI_FIXTURE;
  const raw = fixturePath ? await readJson(fixturePath) : await fetchKimiUsage();

  const windows = [];
  const fiveHour = raw.limits?.find(
    (limit) => Number(limit?.window?.duration) === 300 && limit?.window?.timeUnit === "TIME_UNIT_MINUTE"
  );
  if (fiveHour?.detail) {
    windows.push(windowFromUsage("300-minute", "5h", fiveHour.detail));
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

  console.log(
    JSON.stringify({
      status: "ok",
      updatedAt: new Date().toISOString(),
      windows,
      metadata: {
        region: raw.user?.region ?? null,
        membershipLevel: raw.user?.membership?.level ?? null,
        authMethod: raw.authentication?.method ?? null,
        authScope: raw.authentication?.scope ?? null,
        subType: raw.subType ?? null,
        parallelLimit: numberOrNull(raw.parallel?.limit)
      }
    })
  );
})().catch((error) => {
  console.error(error instanceof Error ? error.message : String(error));
  process.exit(1);
});

async function fetchKimiUsage() {
  const token = process.env.KIMI_API_KEY;
  if (!token) {
    throw new Error("KIMI_API_KEY is required");
  }

  const response = await fetch("https://api.kimi.com/coding/v1/usages", {
    headers: { Authorization: `Bearer ${token}` }
  });
  if (!response.ok) {
    throw new Error(`Kimi usage request failed with ${response.status}`);
  }
  return response.json();
}

async function readJson(filePath) {
  return JSON.parse(await fs.readFile(filePath, "utf8"));
}

function windowFromUsage(id, label, usage) {
  return {
    id,
    label,
    used: numberOrNull(usage.used),
    limit: numberOrNull(usage.limit),
    unit: null,
    resetAt: usage.resetTime ?? null,
    confidence: "exact"
  };
}

function numberOrNull(value) {
  if (value === null || value === undefined || value === "") {
    return null;
  }
  const parsed = Number(value);
  return Number.isFinite(parsed) ? parsed : null;
}
