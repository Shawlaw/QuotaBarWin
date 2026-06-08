import { render, screen } from "@testing-library/react";
import { Summary } from "./Summary";
import type { ForecastWindow } from "../lib/forecast";

test("summary_renders_global_lowest_quota", () => {
  const globalLowest: ForecastWindow = {
    providerId: "bigmodel",
    providerName: "BigModel Coding Plan",
    alertLevel: "critical",
    resetLabel: "resets in 2h 14m",
    suggestion: "Avoid long tasks or large refactors",
    window: {
      id: "5h",
      label: "5-Hour Token Limit",
      used: null,
      limit: null,
      unit: null,
      usedPercent: 95,
      remainingPercent: 5,
      resetAt: null,
      resetText: null,
      confidence: "exact"
    }
  };

  render(<Summary globalLowest={globalLowest} />);

  expect(
    screen.getByText("Lowest quota alert")
  ).toBeInTheDocument();
  expect(
    screen.getByText(
      "Lowest remaining across all providers: BigModel Coding Plan · 5-Hour Token Limit · 5% left"
    )
  ).toBeInTheDocument();
});
