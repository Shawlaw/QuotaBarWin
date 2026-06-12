const https = require("node:https");

const CODEX_USAGE_URL = "https://chatgpt.com/backend-api/wham/usage";

(async function main() {
  const token = process.env.CODEX_ACCESS_TOKEN;
  if (!token) {
    throw new Error("CODEX_ACCESS_TOKEN is required");
  }
  const accountId = process.env.CODEX_ACCOUNT_ID || "";

  const raw = await fetchCodexUsage(token.trim(), accountId.trim());

  const windows = [];
  const primary = raw?.rate_limit?.primary_window;
  if (primary && typeof primary.used_percent === "number") {
    windows.push(windowFromUsage("5h", "5h", primary));
  }

  const secondary = raw?.rate_limit?.secondary_window;
  if (secondary && typeof secondary.used_percent === "number") {
    windows.push(windowFromUsage("weekly", "Weekly limit", secondary));
  }

  console.log(
    JSON.stringify({
      status: windows.length > 0 ? "ok" : "warning",
      updatedAt: new Date().toISOString(),
      windows,
      metadata: {
        planType: raw.plan_type ?? null,
        credits: raw.credits ?? null
      }
    })
  );
})().catch((error) => {
  console.error(error instanceof Error ? error.message : String(error));
  process.exit(1);
});

function fetchCodexUsage(token, accountId) {
  return new Promise((resolve, reject) => {
    const headers = {
      Authorization: `Bearer ${token}`,
      "User-Agent": "QuotaBarWin/0.0"
    };
    if (accountId) {
      headers["ChatGPT-Account-Id"] = accountId;
    }

    const request = https.get(CODEX_USAGE_URL, { headers }, (response) => {
      let body = "";
      response.on("data", (chunk) => {
        body += chunk;
      });
      response.on("end", () => {
        if (response.statusCode !== 200) {
          reject(new Error(`Codex usage API returned ${response.statusCode}: ${body.slice(0, 160)}`));
          return;
        }
        try {
          resolve(JSON.parse(body));
        } catch (error) {
          reject(new Error(`Failed to parse Codex usage response: ${error instanceof Error ? error.message : String(error)}`));
        }
      });
    });

    request.on("error", (error) => reject(error));
    request.setTimeout(15000, () => {
      request.destroy();
      reject(new Error("Codex usage request timed out"));
    });
  });
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

function resetIsoFromWindow(usage) {
  if (typeof usage.reset_at === "number" && usage.reset_at > 0) {
    return new Date(usage.reset_at * 1000).toISOString();
  }
  if (typeof usage.reset_after_seconds === "number" && usage.reset_after_seconds > 0) {
    return new Date(Date.now() + usage.reset_after_seconds * 1000).toISOString();
  }
  return null;
}

function numberOrNull(value) {
  if (value === null || value === undefined || value === "") {
    return null;
  }
  const parsed = Number(value);
  return Number.isFinite(parsed) ? parsed : null;
}
