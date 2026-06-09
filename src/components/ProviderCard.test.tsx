import { fireEvent, render, screen } from "@testing-library/react";
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

  expect(screen.getByText(/Command failed/)).toBeInTheDocument();
});

test("provider_card_renders_remaining_percent", () => {
  render(<ProviderCard provider={provider} />);

  expect(screen.getByText("72% remaining")).toBeInTheDocument();
  expect(screen.queryByText(/28% used/)).not.toBeInTheDocument();
});

test("provider_card_renders_used_percent_when_display_mode_is_used", () => {
  render(<ProviderCard provider={provider} displayMode="used" />);

  expect(screen.getByText("28% used")).toBeInTheDocument();
  expect(screen.queryByText(/72% remaining/)).not.toBeInTheDocument();
});

test("provider_card_fades_used_mode_opacity_as_usage_increases", () => {
  render(<ProviderCard provider={provider} displayMode="used" />);
  const fill = screen.getByRole("progressbar", { name: "Codex Mock 5h window used" }).firstElementChild;

  expect(fill).toHaveStyle({ width: "28%" });
  expect(fill).toHaveStyle({ opacity: "0.352" });
});

test("provider_card_renders_provider_refresh_time", () => {
  vi.spyOn(Date.prototype, "toLocaleString").mockImplementation(function (this: Date) {
    if (this.toISOString() === "2026-06-08T00:00:00.000Z") {
      return "06/08/2026, 08:00 AM GMT+8";
    }
    return "unknown refresh";
  });

  render(<ProviderCard provider={provider} />);

  expect(screen.getByText("Last updated 06/08/2026, 08:00 AM GMT+8")).toBeInTheDocument();
});

test("provider_card_renders_kimi_fixture_snapshot", () => {
  render(<ProviderCard provider={kimiExpected as ProviderSnapshot} />);

  expect(screen.getByRole("heading", { name: "Kimi Coding" })).toBeInTheDocument();
  expect(screen.getByText("Weekly limit")).toBeInTheDocument();
});

test("provider_card_renders_status_label", () => {
  render(<ProviderCard provider={provider} />);

  expect(screen.getByText("status ok")).toBeInTheDocument();
  expect(screen.queryByText("Bottleneck")).not.toBeInTheDocument();
});

test("provider_card_shows_refresh_without_action_menu", () => {
  const onRefresh = vi.fn();

  render(<ProviderCard provider={provider} onRefresh={onRefresh} />);

  fireEvent.click(screen.getByRole("button", { name: "Refresh" }));

  expect(onRefresh).toHaveBeenCalledTimes(1);
  expect(screen.queryByRole("button", { name: "More" })).not.toBeInTheDocument();
});

test("provider_card_opens_warning_status_details", () => {
  render(
    <ProviderCard
      provider={{
        ...provider,
        status: "warning",
        diagnostics: {
          checkedAt: "2026-06-08T00:00:00.000Z",
          messages: ["usage missing"],
          commandPath: "node",
          exitCode: 0,
          durationMs: 42,
          timedOut: false,
          stderr: null
        }
      }}
    />
  );

  fireEvent.click(screen.getByRole("button", { name: "status warning" }));

  expect(screen.getByText("usage missing")).toBeInTheDocument();
  expect(screen.getByText("42ms")).toBeInTheDocument();
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

  expect(screen.getAllByText("resets 06/08/2026, 10:00 AM GMT+8").length).toBeGreaterThan(0);
  expect(screen.getByText("resets 06/30/2026, 10:00 AM GMT+8")).toBeInTheDocument();
});

test("provider_card_collapses_and_expands_many_quota_windows", () => {
  render(
    <ProviderCard
      provider={{
        ...provider,
        windows: Array.from({ length: 7 }, (_, index) => ({
          ...provider.windows[0],
          id: `window-${index + 1}`,
          label: `Window ${index + 1}`
        }))
      }}
    />
  );

  expect(screen.getAllByTestId(/quota-row-codex-mock-window-/)).toHaveLength(4);
  fireEvent.click(screen.getByRole("button", { name: "+ 3 more quota windows" }));
  expect(screen.getAllByTestId(/quota-row-codex-mock-window-/)).toHaveLength(7);
  fireEvent.click(screen.getByRole("button", { name: "Show less" }));
  expect(screen.getAllByTestId(/quota-row-codex-mock-window-/)).toHaveLength(4);
});
