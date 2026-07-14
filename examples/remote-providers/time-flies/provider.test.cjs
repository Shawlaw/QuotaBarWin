const assert = require("node:assert/strict");
const test = require("node:test");
const { buildSnapshot, daysInYear } = require("./provider.cjs");

test("reports local time remaining in whole minutes", () => {
  const now = new Date(2026, 0, 7, 15, 30, 0); // Wednesday, January 7
  const snapshot = buildSnapshot(now);
  const windows = Object.fromEntries(snapshot.windows.map((window) => [window.id, window]));

  assert.equal(snapshot.status, "ok");
  assert.equal(windows.today.remaining, 510);
  assert.equal(windows.today.limit, 1440);
  assert.equal(windows["week-monday"].remaining, 6270);
  assert.equal(windows["week-sunday"].remaining, 4830);
  assert.equal(windows.month.remaining, 35070);
  assert.equal(windows.year.remaining, 516030);
  assert.equal(windows["week-monday"].resetAt, new Date(2026, 0, 12).toISOString());
  assert.equal(windows["week-sunday"].resetAt, new Date(2026, 0, 11).toISOString());
});

test("uses the actual length of leap years", () => {
  assert.equal(daysInYear(2024), 366);
  assert.equal(daysInYear(2025), 365);
});
