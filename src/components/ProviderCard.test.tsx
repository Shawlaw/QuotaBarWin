import { fireEvent, render, screen } from "@testing-library/react";
import { afterEach, vi } from "vitest";
import { ProviderCard } from "./ProviderCard";
import type { ProviderSnapshot } from "../types";
import kimiExpected from "../../fixtures/expected/kimi_provider_snapshot.json";
import { I18nProvider } from "../i18n";
import type { ReactElement } from "react";

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

function renderWithEnglish(ui: ReactElement) {
  return render(ui, {
    wrapper: ({ children }) => (
      <I18nProvider language="en">{children}</I18nProvider>
    )
  });
}

test("ProviderCard renders provider name", () => {
  renderWithEnglish(<ProviderCard provider={provider} />);

  expect(screen.getByRole("heading", { name: "Codex Mock" })).toBeInTheDocument();
});

test("provider_card_renders_error_state", () => {
  renderWithEnglish(<ProviderCard provider={{ ...provider, status: "error", error: "Command failed" }} />);

  expect(screen.getByText(/Command failed/)).toBeInTheDocument();
});

test("provider_card_renders_remaining_percent", () => {
  renderWithEnglish(<ProviderCard provider={provider} />);

  expect(screen.getByText("72% remaining")).toBeInTheDocument();
  expect(screen.queryByText(/28% used/)).not.toBeInTheDocument();
});

test("provider_card_renders_used_percent_when_display_mode_is_used", () => {
  renderWithEnglish(<ProviderCard provider={provider} displayMode="used" />);

  expect(screen.getByText("28% used")).toBeInTheDocument();
  expect(screen.queryByText(/72% remaining/)).not.toBeInTheDocument();
});

test("provider_card_exposes_amount_detail_on_hover", () => {
  renderWithEnglish(
    <ProviderCard
      provider={{
        ...provider,
        windows: [
          {
            ...provider.windows[0],
            id: "balance-cny",
            label: "CNY balance",
            remaining: 12,
            used: 88,
            limit: 100,
            unit: "CNY",
            remainingPercent: 12,
            warningRemaining: 20
          }
        ]
      }}
    />
  );

  expect(screen.getByTestId("quota-row-codex-mock-balance-cny")).toHaveAttribute(
    "title",
    "Remaining 12 CNY / 100 CNY · warning 20 CNY"
  );
});

test("provider_card_fades_used_mode_opacity_as_usage_increases", () => {
  const { rerender } = renderWithEnglish(<ProviderCard provider={provider} displayMode="used" />);
  const fill = screen.getByRole("progressbar", { name: "Codex Mock 5h window used" }).firstElementChild;

  expect(fill).toHaveStyle({ width: "28%" });
  expect(fill).toHaveStyle({ opacity: "0.748" }); // opacity follows remaining percent (72), not used percent (28)

  rerender(
    <ProviderCard
      provider={{
        ...provider,
        windows: [{ ...provider.windows[0], usedPercent: 80, remainingPercent: 20 }]
      }}
      displayMode="used"
    />
  );

  expect(fill).toHaveStyle({ width: "80%" });
  expect(fill).toHaveStyle({ opacity: "0.28" }); // 0.1 + 20 * 0.009
});

test("provider_card_renders_provider_refresh_time", () => {
  vi.spyOn(Date.prototype, "toLocaleString").mockImplementation(function (this: Date) {
    if (this.toISOString() === "2026-06-08T00:00:00.000Z") {
      return "06/08/2026, 08:00 AM GMT+8";
    }
    return "unknown refresh";
  });

  renderWithEnglish(<ProviderCard provider={provider} />);

  expect(screen.getByText("Last updated 06/08/2026, 08:00 AM GMT+8")).toBeInTheDocument();
});

test("provider_card_renders_kimi_fixture_snapshot", () => {
  renderWithEnglish(<ProviderCard provider={kimiExpected as ProviderSnapshot} />);

  expect(screen.getByRole("heading", { name: "Kimi Coding" })).toBeInTheDocument();
  expect(screen.getByText("Weekly limit")).toBeInTheDocument();
});

test("provider_card_renders_status_label", () => {
  renderWithEnglish(<ProviderCard provider={provider} />);

  expect(screen.getByText("status ok")).toBeInTheDocument();
  expect(screen.queryByText("Bottleneck")).not.toBeInTheDocument();
});

test("provider_card_shows_refresh_without_action_menu", () => {
  const onRefresh = vi.fn();

  renderWithEnglish(<ProviderCard provider={provider} onRefresh={onRefresh} />);

  fireEvent.click(screen.getByRole("button", { name: "Refresh" }));

  expect(onRefresh).toHaveBeenCalledTimes(1);
  expect(screen.queryByRole("button", { name: "More" })).not.toBeInTheDocument();
});

test("provider_card_opens_warning_status_details", () => {
  renderWithEnglish(
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

  renderWithEnglish(
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

  expect(screen.getByText("resets 06/08/2026, 10:00 AM GMT+8")).toBeInTheDocument();
  expect(screen.getByText("resets 06/30/2026, 10:00 AM GMT+8")).toBeInTheDocument();
});

test("provider_card_collapses_and_expands_many_quota_windows", () => {
  renderWithEnglish(
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

test('provider_card_renders_stale_state_with_cached_data', () => {
  renderWithEnglish(
    <ProviderCard
      provider={{
        ...provider,
        status: 'stale',
        error: 'Refresh failed · Connection timed out',
        updatedAt: null
      }}
    />
  );

  expect(screen.getByText('status stale')).toBeInTheDocument();
  expect(screen.getByText(/Connection timed out/)).toBeInTheDocument();
  expect(screen.getByText('72% remaining')).toBeInTheDocument();
});
