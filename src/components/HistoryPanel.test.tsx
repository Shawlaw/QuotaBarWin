import { fireEvent, render, screen } from "@testing-library/react";
import { vi } from "vitest";
import { HistoryPanel } from "./HistoryPanel";
import { I18nProvider } from "../i18n";
import type { QuotaEvent } from "../types";

function event(overrides: Partial<QuotaEvent> = {}): QuotaEvent {
  return {
    id: 1,
    occurredAt: "2026-10-05T12:00:00Z",
    eventType: "quota-reset",
    severity: "positive",
    providerId: "remote-a",
    providerName: "Remote A",
    windowId: "5h",
    windowLabel: "5h window",
    details: { remainingPercent: 95 },
    ...overrides,
  };
}

function renderPanel(props: Partial<Parameters<typeof HistoryPanel>[0]> = {}) {
  const onRefresh = vi.fn();
  const onClear = vi.fn();
  render(
    <I18nProvider language="en">
      <HistoryPanel
        events={[]}
        loadFailed={false}
        onRefresh={onRefresh}
        onClear={onClear}
        {...props}
      />
    </I18nProvider>,
  );
  return { onRefresh, onClear };
}

describe("HistoryPanel", () => {
  test("renders event messages with provider and window context", () => {
    renderPanel({
      events: [
        event(),
        event({
          id: 2,
          eventType: "provider-error",
          severity: "error",
          windowId: null,
          windowLabel: null,
          details: { status: "error", error: "timed out" },
        }),
      ],
    });

    expect(screen.getByText(/Remote A 5h window quota reset/i)).toBeInTheDocument();
    expect(screen.getByText(/Remote A refresh failed: timed out/i)).toBeInTheDocument();
    expect(screen.getAllByTestId("history-item")).toHaveLength(2);
  });

  test("renders quota-reset-time-changed with the next expiry time", () => {
    renderPanel({
      events: [
        event({
          eventType: "quota-reset-time-changed",
          severity: "info",
          details: { resetAtBefore: "2026-10-05T11:00:00Z", resetAt: "2026-10-05T16:00:00Z" },
        }),
      ],
    });

    expect(
      screen.getByText(/Remote A 5h window quota expiry time changed \(next .*\)/i),
    ).toBeInTheDocument();

    fireEvent.click(screen.getByTestId("history-filter-quota"));
    expect(screen.getAllByTestId("history-item")).toHaveLength(1);
  });

  test("filters events by category", () => {
    renderPanel({
      events: [
        event(),
        event({ id: 2, eventType: "app-update-applied", severity: "positive", providerId: null, providerName: null, windowId: null, windowLabel: null, details: { version: "1.6.0" } }),
      ],
    });

    fireEvent.click(screen.getByTestId("history-filter-app"));
    expect(screen.getAllByTestId("history-item")).toHaveLength(1);
    expect(screen.getByText(/Updated to 1\.6\.0/i)).toBeInTheDocument();

    fireEvent.click(screen.getByTestId("history-filter-quota"));
    expect(screen.getAllByTestId("history-item")).toHaveLength(1);
    expect(screen.getByText(/Remote A 5h window quota reset/i)).toBeInTheDocument();

    fireEvent.click(screen.getByTestId("history-filter-all"));
    expect(screen.getAllByTestId("history-item")).toHaveLength(2);
  });

  test("shows the empty state when there are no events", () => {
    renderPanel({ events: [] });
    expect(screen.getByTestId("history-empty")).toBeInTheDocument();
  });

  test("shows loading and failure states", () => {
    const { rerender } = render(
      <I18nProvider language="en">
        <HistoryPanel events={null} loadFailed={false} onRefresh={() => undefined} onClear={() => undefined} />
      </I18nProvider>,
    );
    expect(screen.getByTestId("history-loading")).toBeInTheDocument();

    rerender(
      <I18nProvider language="en">
        <HistoryPanel events={[]} loadFailed={true} onRefresh={() => undefined} onClear={() => undefined} />
      </I18nProvider>,
    );
    expect(screen.getByTestId("history-load-failed")).toBeInTheDocument();
  });

  test("clear asks for confirmation before clearing", async () => {
    const { onClear } = renderPanel({ events: [event()] });

    fireEvent.click(screen.getByTestId("history-clear"));
    expect(onClear).not.toHaveBeenCalled();
    expect(
      await screen.findByRole("dialog", { name: "Clear event history?" }),
    ).toBeInTheDocument();

    fireEvent.click(screen.getByTestId("confirm-clear-history"));
    expect(onClear).toHaveBeenCalled();
  });

  test("clear is disabled when the history is empty", () => {
    renderPanel({ events: [] });
    expect(screen.getByTestId("history-clear")).toBeDisabled();
  });

  test("app started events show the version with short commit", () => {
    renderPanel({
      events: [
        event({
          id: 1,
          eventType: "app-started",
          severity: "info",
          providerId: null,
          providerName: null,
          windowId: null,
          windowLabel: null,
          details: { startedHidden: false, version: "1.6.0(a1b2c3d)" },
        }),
        event({
          id: 2,
          eventType: "app-started",
          severity: "info",
          providerId: null,
          providerName: null,
          windowId: null,
          windowLabel: null,
          details: { startedHidden: true, version: "1.6.0(a1b2c3d)" },
        }),
      ],
    });

    expect(screen.getByText(/Application started \(version 1\.6\.0\(a1b2c3d\)\)/i)).toBeInTheDocument();
    expect(
      screen.getByText(/Application started in the background \(version 1\.6\.0\(a1b2c3d\)\)/i),
    ).toBeInTheDocument();
  });
});
