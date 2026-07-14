const PROVIDER_ID = process.env.QBWIN_PROVIDER_ID || "time-flies";
const MINUTE_MS = 60 * 1000;
const DAY_MS = 24 * 60 * MINUTE_MS;

function buildSnapshot(now = new Date()) {
  if (!Number.isFinite(now.getTime())) {
    throw new Error("A valid local date is required");
  }

  const tomorrow = startOfNextLocalDay(now);
  const nextMonday = nextLocalWeekStart(now, 1);
  const nextSunday = nextLocalWeekStart(now, 0);
  const nextMonth = new Date(now.getFullYear(), now.getMonth() + 1, 1);
  const nextYear = new Date(now.getFullYear() + 1, 0, 1);

  return {
    status: "ok",
    updatedAt: now.toISOString(),
    windows: [
      timeWindow("today", "Today remaining", now, tomorrow, DAY_MS / MINUTE_MS, "minutes"),
      timeWindow("week-monday", "Week remaining (Monday start)", now, nextMonday, (7 * DAY_MS) / MINUTE_MS, "minutes"),
      timeWindow("week-sunday", "Week remaining (Sunday start)", now, nextSunday, (7 * DAY_MS) / MINUTE_MS, "minutes"),
      timeWindow(
        "month",
        "Month remaining",
        now,
        nextMonth,
        (daysInMonth(now.getFullYear(), now.getMonth()) * DAY_MS) / MINUTE_MS,
        "minutes"
      ),
      timeWindow(
        "year",
        "Year remaining",
        now,
        nextYear,
        (daysInYear(now.getFullYear()) * DAY_MS) / MINUTE_MS,
        "minutes"
      )
    ],
    metadata: {
      providerId: PROVIDER_ID,
      timezone: Intl.DateTimeFormat().resolvedOptions().timeZone || null,
      precision: "minutes"
    }
  };
}

function timeWindow(id, label, now, resetAt, limit, unit) {
  const millisecondsRemaining = Math.max(0, resetAt.getTime() - now.getTime());
  const remaining = Math.ceil(millisecondsRemaining / MINUTE_MS);
  const used = Math.max(0, limit - remaining);
  const remainingPercent = Math.max(0, Math.min(100, (remaining / limit) * 100));

  return {
    id,
    label,
    remaining,
    used,
    limit,
    unit,
    usedPercent: 100 - remainingPercent,
    remainingPercent,
    resetAt: resetAt.toISOString(),
    resetText: null,
    confidence: "exact"
  };
}

function startOfNextLocalDay(now) {
  return new Date(now.getFullYear(), now.getMonth(), now.getDate() + 1);
}

function nextLocalWeekStart(now, weekStartDay) {
  const daysUntilStart = (weekStartDay - now.getDay() + 7) % 7 || 7;
  return new Date(now.getFullYear(), now.getMonth(), now.getDate() + daysUntilStart);
}

function daysInMonth(year, month) {
  return new Date(year, month + 1, 0).getDate();
}

function daysInYear(year) {
  return new Date(year, 11, 31).getDate() === 31 && new Date(year, 1, 29).getDate() === 29 ? 366 : 365;
}

if (require.main === module) {
  console.log(JSON.stringify(buildSnapshot()));
}

module.exports = {
  buildSnapshot,
  daysInYear,
  nextLocalWeekStart,
  startOfNextLocalDay
};
