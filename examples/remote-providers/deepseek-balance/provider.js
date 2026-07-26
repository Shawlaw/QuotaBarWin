function main(qb) {
  logStep(qb, "info", "provider.start", "DeepSeek provider started");
  const raw = fetchDeepSeekBalance(qb);
  const requestedCurrency = textOrNull(qb.env.getOptional("DEEPSEEK_BALANCE_CURRENCY"));
  const balances = (raw.balance_infos || []).filter(
    (balance) => !requestedCurrency || balance.currency === requestedCurrency
  );
  if (requestedCurrency && balances.length === 0) {
    throw new Error(`DeepSeek balance did not include currency ${requestedCurrency}`);
  }
  const windows = balances.map((balance) => balanceToWindow(qb, balance));
  const warningWindows = windows.filter(
    (window) => window.warningRemaining !== null && window.remaining !== null
      && window.remaining <= window.warningRemaining
  );
  const status = raw.is_available === false || warningWindows.length > 0 ? "warning" : "ok";
  logStep(qb, "info", "snapshot.ready", "DeepSeek snapshot ready", {
    status,
    windowCount: windows.length,
    warningWindowCount: warningWindows.length
  });
  const balancesMetadata = {};
  for (const balance of balances) {
    balancesMetadata[balance.currency] = {
      totalBalance: numberOrNull(balance.total_balance),
      grantedBalance: numberOrNull(balance.granted_balance),
      toppedUpBalance: numberOrNull(balance.topped_up_balance)
    };
  }
  return {
    status,
    updatedAt: qb.now(),
    windows,
    diagnostics: raw.is_available === false || warningWindows.length > 0 ? {
      checkedAt: qb.now(),
      messages: (raw.is_available === false ? ["DeepSeek account is not available"] : []).concat(
        warningWindows.map(
          (window) => `${window.label} is at ${formatMoney(window.remaining, window.unit)}; warning ${formatMoney(window.warningRemaining, window.unit)}`
        )
      )
    } : null,
    metadata: {
      isAvailable: raw.is_available === undefined ? null : raw.is_available,
      currency: requestedCurrency,
      balances: balancesMetadata
    }
  };
}

function fetchDeepSeekBalance(qb) {
  const token = qb.env.get("DEEPSEEK_API_KEY");
  logStep(qb, "info", "http.request.start", "Requesting DeepSeek balance", {
    endpoint: "api.deepseek.com/user/balance"
  });
  const response = qb.http.request("https://api.deepseek.com/user/balance", {
    headers: { Accept: "application/json", Authorization: `Bearer ${token}` }
  });
  logStep(qb, response.ok ? "info" : "warn", "http.response", "DeepSeek balance response received", {
    status: response.status
  });
  if (!response.ok) throw new Error(`DeepSeek balance request failed with ${response.status}`);
  return JSON.parse(response.body);
}

function logStep(qb, level, stage, message, fields) {
  qb.log(Object.assign({ level, stage, message }, fields || {}));
}

function balanceToWindow(qb, balance) {
  const currency = String(balance.currency || "unknown").trim() || "unknown";
  const remaining = numberOrNull(balance.total_balance);
  const referenceTotal = envNumber(qb, `DEEPSEEK_BALANCE_REFERENCE_TOTAL_${currency}`)
    ?? envNumber(qb, "DEEPSEEK_BALANCE_REFERENCE_TOTAL");
  const warningRemaining = envNumber(qb, `DEEPSEEK_BALANCE_WARNING_${currency}`)
    ?? envNumber(qb, "DEEPSEEK_BALANCE_WARNING");
  const used = referenceTotal !== null && remaining !== null ? Math.max(0, referenceTotal - remaining) : null;
  return {
    id: `balance-${dash(currency)}`,
    label: `${currency} balance`,
    remaining,
    used,
    limit: referenceTotal,
    unit: currency,
    warningRemaining,
    resetAt: null,
    resetText: remaining === null ? null : `Balance ${formatMoney(remaining, currency)}`,
    confidence: "exact"
  };
}

function envNumber(qb, name) { return numberOrNull(qb.env.getOptional(name)); }
function numberOrNull(value) {
  if (value === null || value === undefined || value === "") return null;
  const parsed = Number(value);
  return Number.isFinite(parsed) ? parsed : null;
}
function textOrNull(value) { const text = String(value || "").trim(); return text || null; }
function dash(value) { return String(value || "unknown").trim().replace(/[^A-Za-z0-9]+/g, "-").replace(/^-+|-+$/g, "").toLowerCase(); }
function formatMoney(value, currency) {
  if (value === null || value === undefined || !Number.isFinite(value)) return `unknown ${currency || ""}`.trim();
  return `${Number.isInteger(value) ? value : value.toFixed(2)} ${currency || ""}`.trim();
}
