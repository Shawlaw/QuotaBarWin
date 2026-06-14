import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
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
    providers: []
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
          }
        ]
      }
    ]
  };

  return {
    getCachedSnapshot: vi.fn(async () => null),
    getConfig: vi.fn(async () => config),
    hideCurrentWindow: vi.fn(async () => undefined),
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

  await act(async () => {
    mocks.listeners.trayShown?.();
  });

  await waitFor(() => expect(mocks.refreshSnapshot).toHaveBeenCalledTimes(2));
});

test("tray_popup_hides_current_window_on_escape", async () => {
  render(<TrayPopup />);

  fireEvent.keyDown(window, { key: "Escape" });

  await waitFor(() => expect(mocks.hideCurrentWindow).toHaveBeenCalledTimes(1));
});
