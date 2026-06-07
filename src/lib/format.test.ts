import { formatPercent } from "./format";

test("formatPercent handles null", () => {
  expect(formatPercent(null)).toBe("Unknown");
});
