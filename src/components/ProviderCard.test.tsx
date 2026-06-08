import { render, screen } from "@testing-library/react";
import { afterEach, vi } from "vitest";
import { ProviderCard } from "./ProviderCard";
import type { ProviderSnapshot } from "../types";
import kimiExpected from "../../docs/specs/fixtures/expected/kimi_provider_snapshot.json";

afterEach(() => {
  vi.restoreAllMocks();
});

const provider: ProviderSnapshot = {
  id: "codex-mock",
  name: "Codex Mock",
  status: "ok",
  source: "mock",
  updatedAt: "2026-06-08T00:00:00.000Z",
  error: null,
  diagnostics: null,
  metadata: null,
  windows: [
    {
      id: "5h",
      label: "5h window",
      used: 28,
      limit: 100,
      unit: "percent",
      usedPercent: 28,
      remainingPercent: 72,
      resetAt: null,
      resetText: null,
      confidence: "estimated"
    }
  ]
};

test("ProviderCard renders provider name", () => {
  render(<ProviderCard provider={provider} />);

  expect(screen.getByRole("heading", { name: "Codex Mock" })).toBeInTheDocument();
});

test("provider_card_renders_error_state", () => {
  render(<ProviderCard provider={{ ...provider, status: "error", error: "Command failed" }} />);

  expect(screen.getByText("Command failed")).toBeInTheDocument();
});

test("provider_card_renders_used_and_remaining", () => {
  render(<ProviderCard provider={provider} />);

  expect(screen.getByText("28% used / 72% remaining")).toBeInTheDocument();
});

test("provider_card_renders_kimi_fixture_snapshot", () => {
  render(<ProviderCard provider={kimiExpected as ProviderSnapshot} />);

  expect(screen.getByRole("heading", { name: "Kimi Coding" })).toBeInTheDocument();
  expect(screen.getByText("Coding usage")).toBeInTheDocument();
});

test("provider_card_renders_bottleneck_badge", () => {
  render(<ProviderCard provider={provider} />);

  expect(screen.getAllByText("Bottleneck").length).toBeGreaterThan(0);
});

test("provider_card_renders_suggestion_text", () => {
  render(
    <ProviderCard
      provider={{
        ...provider,
        windows: [
          {
            ...provider.windows[0],
            remainingPercent: 8,
            usedPercent: 92,
            resetText: "in 2 hours"
          }
        ]
      }}
    />
  );

  expect(screen.getByText(/Avoid long tasks or large refactors/)).toBeInTheDocument();
});

test("provider_card_renders_reset_time_for_each_window", () => {
  vi.spyOn(Date.prototype, "toLocaleString").mockImplementation(function (this: Date) {
    if (this.toISOString() === "2026-06-08T02:00:00.000Z") {
      return "06/08/2026, 10:00 AM GMT+8";
    }
    if (this.toISOString() === "2026-06-30T02:00:00.000Z") {
      return "06/30/2026, 10:00 AM GMT+8";
    }
    return "unknown reset";
  });

  render(
    <ProviderCard
      provider={{
        ...provider,
        windows: [
          {
            ...provider.windows[0],
            id: "hourly",
            label: "1 hour limit",
            resetAt: "2026-06-08T02:00:00Z"
          },
          {
            ...provider.windows[0],
            id: "monthly",
            label: "1 month limit",
            resetAt: "2026-06-30T02:00:00Z"
          }
        ]
      }}
    />
  );

  expect(screen.getAllByText("resets at 06/08/2026, 10:00 AM GMT+8").length).toBeGreaterThan(0);
  expect(screen.getByText("resets at 06/30/2026, 10:00 AM GMT+8")).toBeInTheDocument();
});
