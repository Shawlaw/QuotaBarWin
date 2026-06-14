import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { TrayPopup } from "./TrayPopup";
import type { AppConfig, AppSnapshot } from "../types";

const mocks = vi.hoisted(() => {
  const listeners: { trayShown?: () => void } = {};
  const config: AppConfig = {
    schemaVersion: 8,
    refreshIntervalSeconds: 300,
    displayMode: "remaining",
    lowQuotaWarningThreshold: 20,
    language: "system",
    providers: [
      {
        id: "remote-kimi",
        name: "Kimi",
        enabled: true,
        kind: "remote",
        manifestUrl: "https://example.test/kimi/provider.json",
        sourceUrl: "https://example.test/kimi/provider.cjs",
        runtime: "node",
        autoUpdate: false,
        updateIntervalSeconds: 3600
      },
      {
        id: "mock-codex",
        name: "Codex Mock",
        enabled: true,
        kind: "mock"
      }
    ]
  };

  const snapshot: AppSnapshot = {
    schemaVersion: 1,
    refreshedAt: "2026-06-08T10:00:00+08:00",
    providers: [
      {
        id: "mock-codex",
        name: "Codex Mock",
        status: "ok",
        source: "mock",
        updatedAt: "2026-06-08T10:00:00+08:00",
        error: null,
        diagnostics: null,
        metadata: null,
        windows: [
          {
            id: "daily",
            label: "Daily",
            used: 72,
            limit: 100,
            unit: "percent",
            usedPercent: 72,
            remainingPercent: 28,
            resetAt: null,
            resetText: "resets tomorrow",
            confidence: "estimated"
          },
          {
            id: "weekly",
            label: "Weekly",
            used: 12,
            limit: 100,
            unit: "percent",
            usedPercent: 12,
            remainingPercent: 88,
            resetAt: null,
            resetText: "resets Friday",
            confidence: "estimated"
          },
          {
            id: "monthly",
            label: "Monthly",
            used: 51,
            limit: 100,
            unit: "percent",
            usedPercent: 51,
            remainingPercent: 49,
            resetAt: null,
            resetText: "resets month end",
            confidence: "estimated"
          },
          {
            id: "token",
            label: "Token pool",
            used: 5,
            limit: 100,
            unit: "percent",
            usedPercent: 5,
            remainingPercent: 95,
            resetAt: null,
            resetText: "resets soon",
            confidence: "estimated"
          },
          {
            id: "extra",
            label: "Extra window",
            used: 90,
            limit: 100,
            unit: "percent",
            usedPercent: 90,
            remainingPercent: 10,
            resetAt: null,
            resetText: "resets later",
            confidence: "estimated"
          }
        ]
      },
      {
        id: "remote-kimi",
        name: "Kimi",
        status: "ok",
        source: "remote",
        updatedAt: "2026-06-08T10:00:00+08:00",
        error: null,
        diagnostics: null,
        metadata: null,
        windows: [
          {
            id: "kimi-daily",
            label: "Kimi Daily",
            used: 40,
            limit: 100,
            unit: "percent",
            usedPercent: 40,
            remainingPercent: 60,
            resetAt: null,
            resetText: "resets tonight",
            confidence: "estimated"
          }
        ]
      }
    ]
  };

  return {
    getCachedSnapshot: vi.fn(async () => null),
    getConfig: vi.fn(async () => config),
    hideCurrentWindow: vi.fn(async () => undefined),
    hideTrayPopup: vi.fn(async () => undefined),
    listenForTrayPopupShown: vi.fn(async (callback: () => void) => {
      listeners.trayShown = callback;
      return () => undefined;
    }),
    listeners,
    refreshSnapshot: vi.fn(async () => snapshot)
  };
});

vi.mock("../lib/api", () => mocks);

test("tray_popup_loads_snapshot_and_refreshes_when_shown", async () => {
  render(<TrayPopup />);

  await waitFor(() => expect(mocks.refreshSnapshot).toHaveBeenCalledTimes(1));
  expect(screen.getByTestId("tray-popup")).toBeInTheDocument();
  expect(screen.getByText("Codex Mock")).toBeInTheDocument();
  expect(screen.getByText("28% remaining")).toBeInTheDocument();
  expect(screen.getByText("Extra window")).toBeInTheDocument();

  await act(async () => {
    mocks.listeners.trayShown?.();
  });

  await waitFor(() => expect(mocks.refreshSnapshot).toHaveBeenCalledTimes(2));
});

test("tray_popup_hides_popup_on_escape_and_close_button", async () => {
  render(<TrayPopup />);

  fireEvent.keyDown(window, { key: "Escape" });
  fireEvent.click(screen.getByRole("button", { name: "Close" }));

  await waitFor(() => expect(mocks.hideTrayPopup).toHaveBeenCalledTimes(2));
  expect(mocks.hideCurrentWindow).not.toHaveBeenCalled();
});

test("tray_popup_follows_configured_provider_order", async () => {
  render(<TrayPopup />);

  await waitFor(() => expect(mocks.refreshSnapshot).toHaveBeenCalled());
  const providerStatus = screen.getByLabelText("Provider status");
  const providerHeadings = within(providerStatus)
    .getAllByText(/Kimi|Codex Mock/)
    .map((element) => element.textContent);

  expect(providerHeadings[0]).toBe("Kimi");
  expect(providerHeadings[1]).toBe("Codex Mock");
});
