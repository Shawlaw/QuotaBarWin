const fs = require("node:fs/promises");

const PROVIDER_ID = process.env.QBWIN_PROVIDER_ID || "deepseek-balance";

(async function main() {
  logStep("info", "provider.start", "DeepSeek provider started");
  const fixturePath = process.env.QUOTABARWIN_DEEPSEEK_FIXTURE;
  const raw = fixturePath ? await readJsonFixture(fixturePath) : await fetchDeepSeekBalance();

  // Raw DeepSeek shape used here:
  // {
  //   is_available: true,
  //   balance_infos: [{ currency, total_balance, granted_balance, topped_up_balance }]
  // }
  // The API reports the current money balance, not a plan quota. A local
  // reference total can be provided to render a progress bar, and a local
  // warning balance can trigger low-balance warnings.
  const requestedCurrency = textOrNull(process.env.DEEPSEEK_BALANCE_CURRENCY);
  const balances = (raw.balance_infos ?? [])
    .filter((balance) => !requestedCurrency || balance.currency === requestedCurrency);
  logStep("info", "balance.filter", "DeepSeek balances filtered", {
    requestedCurrency: requestedCurrency ?? null,
    balanceCount: balances.length
  });

  if (requestedCurrency && balances.length === 0) {
    throw new Error(`DeepSeek balance did not include currency ${requestedCurrency}`);
  }

  const windows = balances.map(balanceToWindow);
  const warningWindows = windows.filter(
    (window) =>
      window.warningRemaining !== null &&
      window.warningRemaining !== undefined &&
      window.remaining !== null &&
      window.remaining !== undefined &&
      window.remaining <= window.warningRemaining
  );
  logStep("info", "snapshot.ready", "DeepSeek snapshot ready", {
    status: raw.is_available === false || warningWindows.length > 0 ? "warning" : "ok",
    windowCount: windows.length,
    warningWindowCount: warningWindows.length
  });

  console.log(
    JSON.stringify({
      status: raw.is_available === false || warningWindows.length > 0 ? "warning" : "ok",
      updatedAt: new Date().toISOString(),
      windows,
      diagnostics:
        raw.is_available === false || warningWindows.length > 0
          ? {
              checkedAt: new Date().toISOString(),
              messages: [
                ...(raw.is_available === false ? ["DeepSeek account is not available"] : []),
                ...warningWindows.map(
                  (window) =>
                    `${window.label} is at ${formatMoney(window.remaining, window.unit)}; warning ${formatMoney(
                      window.warningRemaining,
                      window.unit
                    )}`
                )
              ]
            }
          : null,
      metadata: {
        isAvailable: raw.is_available ?? null,
        currency: requestedCurrency ?? null,
        balances: Object.fromEntries(
          balances.map((balance) => [
            balance.currency,
            {
              totalBalance: numberOrNull(balance.total_balance),
              grantedBalance: numberOrNull(balance.granted_balance),
              toppedUpBalance: numberOrNull(balance.topped_up_balance)
            }
          ])
        )
      }
    })
  );
})().catch((error) => {
  logStep("error", "provider.error", "DeepSeek provider failed", {
    error: error instanceof Error ? error.message : String(error)
  });
  console.error(error instanceof Error ? error.message : String(error));
  process.exit(1);
});

async function fetchDeepSeekBalance() {
  logStep("info", "auth.check", "Checking DeepSeek API key");
  const token = process.env.DEEPSEEK_API_KEY;
  if (!token) {
    logStep("error", "auth.missing", "DEEPSEEK_API_KEY is missing");
    throw new Error("DEEPSEEK_API_KEY is required");
  }

  logStep("info", "http.request.start", "Requesting DeepSeek balance", {
    endpoint: "api.deepseek.com/user/balance"
  });
  const response = await fetch("https://api.deepseek.com/user/balance", {
    headers: {
      Accept: "application/json",
      Authorization: `Bearer ${token}`
    }
  });
  logStep(response.ok ? "info" : "warn", "http.response", "DeepSeek balance response received", {
    status: response.status
  });
  if (!response.ok) {
    throw new Error(`DeepSeek balance request failed with ${response.status}`);
  }
  const raw = await response.json();
  logStep("info", "http.parse.done", "DeepSeek balance response parsed");
  return raw;
}

async function readJsonFixture(filePath) {
  logStep("info", "fixture.read.start", "Reading DeepSeek fixture");
  const raw = JSON.parse(await fs.readFile(filePath, "utf8"));
  logStep("info", "fixture.read.done", "DeepSeek fixture parsed");
  return raw;
}

function logStep(level, stage, message, fields = {}) {
  process.stderr.write(
    `${JSON.stringify({
      level,
      providerId: PROVIDER_ID,
      version: process.env.QBWIN_PROVIDER_VERSION || null,
      sourceChecksum: process.env.QBWIN_PROVIDER_SOURCE_CHECKSUM || null,
      stage,
      message,
      ...fields
    })}\n`
  );
}

function balanceToWindow(balance) {
  const currency = String(balance.currency ?? "unknown").trim() || "unknown";
  const remaining = numberOrNull(balance.total_balance);
  const referenceTotal = envNumber(`DEEPSEEK_BALANCE_REFERENCE_TOTAL_${currency}`)
    ?? envNumber("DEEPSEEK_BALANCE_REFERENCE_TOTAL");
  const warningRemaining = envNumber(`DEEPSEEK_BALANCE_WARNING_${currency}`)
    ?? envNumber("DEEPSEEK_BALANCE_WARNING");
  const used =
    referenceTotal !== null && remaining !== null
      ? Math.max(0, referenceTotal - remaining)
      : null;

  return {
    id: `balance-${dash(currency)}`,
    label: `${currency} balance`,
    remaining,
    used,
    limit: referenceTotal,
    unit: currency,
    warningRemaining,
    resetAt: null,
    resetText:
      remaining === null
        ? null
        : `Balance ${formatMoney(remaining, currency)}`,
    confidence: "exact"
  };
}

function envNumber(name) {
  return numberOrNull(process.env[name]);
}

function numberOrNull(value) {
  if (value === null || value === undefined || value === "") {
    return null;
  }
  const parsed = Number(value);
  return Number.isFinite(parsed) ? parsed : null;
}

function textOrNull(value) {
  const text = String(value ?? "").trim();
  return text ? text : null;
}

function dash(value) {
  return String(value ?? "unknown")
    .trim()
    .replace(/[^A-Za-z0-9]+/g, "-")
    .replace(/^-+|-+$/g, "")
    .toLowerCase();
}

function formatMoney(value, currency) {
  if (value === null || value === undefined || !Number.isFinite(value)) {
    return `unknown ${currency ?? ""}`.trim();
  }
  return `${Number.isInteger(value) ? value : value.toFixed(2)} ${currency ?? ""}`.trim();
}
