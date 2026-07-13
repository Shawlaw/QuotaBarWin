const assert = require("node:assert/strict");
const test = require("node:test");

const { normalizeRateLimitWindows } = require("./provider.cjs");

function usage(usedPercent, limitWindowSeconds) {
  return { used_percent: usedPercent, limit_window_seconds: limitWindowSeconds };
}

test("normalizes swapped Codex primary and secondary windows by duration", () => {
  const weekly = usage(61, 7 * 24 * 60 * 60);
  const session = usage(24, 5 * 60 * 60);

  const normalized = normalizeRateLimitWindows({
    primary_window: weekly,
    secondary_window: session
  });

  assert.equal(normalized.session, session);
  assert.equal(normalized.weekly, weekly);
});

test("keeps the legacy primary and secondary mapping without duration", () => {
  const primary = { used_percent: 24 };
  const secondary = { used_percent: 61 };

  const normalized = normalizeRateLimitWindows({
    primary_window: primary,
    secondary_window: secondary
  });

  assert.equal(normalized.session, primary);
  assert.equal(normalized.weekly, secondary);
});
