import {
  act,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
import { TrayPopup } from "./TrayPopup";
import type { AppConfig, AppSnapshot } from "../types";
import { I18nProvider } from "../i18n";
import type { ReactElement } from "react";

const mocks = vi.hoisted(() => {
  const listeners: {
    snapshotUpdated?: (snapshot: AppSnapshot) => void;
    trayShown?: (presentationId: number) => void;
  } = {};
  const state = { presentationId: 0 };
  const config: AppConfig = {
    schemaVersion: 14,
    refreshIntervalSeconds: 300,
    displayMode: "remaining",
    lowQuotaWarningThreshold: 20,
    language: "system",
    remoteProviderRegistry: {
      registryUrl: null,
      providerProxyUrl: null,
      autoUpdate: true,
    },
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
        updateIntervalSeconds: 3600,
        timeoutSeconds: 30,
      },
    ],
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
            confidence: "estimated",
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
            confidence: "estimated",
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
            confidence: "estimated",
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
            confidence: "estimated",
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
            confidence: "estimated",
          },
        ],
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
            confidence: "estimated",
          },
        ],
      },
    ],
  };

  return {
    getCachedSnapshot: vi.fn(async (): Promise<AppSnapshot | null> => null),
    getAppVersion: vi.fn(async () => "1.0.0(abc1234)"),
    getConfig: vi.fn(async () => config),
    getTrayPopupPresentationId: vi.fn(async () => state.presentationId),
    hideCurrentWindow: vi.fn(async () => undefined),
    hideTrayPopup: vi.fn(async () => undefined),
    listenForSnapshotUpdates: vi.fn(async (callback: (snapshot: AppSnapshot) => void) => {
      listeners.snapshotUpdated = callback;
      return () => undefined;
    }),
    listenForTrayPopupShown: vi.fn(async (callback: (presentationId: number) => void) => {
      listeners.trayShown = callback;
      return () => undefined;
    }),
    listeners,
    resetTrayPopupSize: vi.fn(async () => undefined),
    refreshSnapshot: vi.fn(async () => snapshot),
    state,
    startDraggingCurrentWindow: vi.fn(async () => undefined),
    startResizingCurrentWindow: vi.fn(async () => undefined),
  };
});

vi.mock("../lib/api", () => mocks);

beforeEach(() => {
  vi.clearAllMocks();
  mocks.listeners.snapshotUpdated = undefined;
  mocks.listeners.trayShown = undefined;
  mocks.state.presentationId = 0;
});

function renderWithEnglish(ui: ReactElement) {
  return render(ui, {
    wrapper: ({ children }) => (
      <I18nProvider language="en">{children}</I18nProvider>
    )
  });
}

test("tray_popup_loads_snapshot_and_refreshes_when_shown", async () => {
  renderWithEnglish(<TrayPopup />);

  await waitFor(() => expect(mocks.refreshSnapshot).toHaveBeenCalledTimes(1));
  expect(screen.getByTestId("tray-popup")).toBeInTheDocument();
  expect(screen.getByText(/Last refreshed at/)).toBeInTheDocument();
  expect(screen.getByText("28% remaining")).toBeInTheDocument();
  expect(screen.getByText("Codex Mock")).toBeInTheDocument();
  expect(screen.getByText("Extra window")).toBeInTheDocument();
  expect(screen.getByText("Kimi")).toBeInTheDocument();
  expect(screen.getByText("Kimi Daily")).toBeInTheDocument();
  expect(screen.getByText("v1.0.0")).toHaveAttribute("title", "Version 1.0.0(abc1234)");
  expect(screen.queryByText(/abc1234/)).not.toBeInTheDocument();
  expect(screen.queryByLabelText("Provider status")).not.toBeInTheDocument();

  await act(async () => {
    mocks.listeners.trayShown?.(1);
  });

  await waitFor(() => expect(mocks.refreshSnapshot).toHaveBeenCalledTimes(2));

  await act(async () => {
    mocks.listeners.trayShown?.(2);
  });

  await waitFor(() => expect(mocks.refreshSnapshot).toHaveBeenCalledTimes(3));

  await act(async () => {
    mocks.listeners.trayShown?.(2);
  });

  expect(mocks.refreshSnapshot).toHaveBeenCalledTimes(3);
});

test("tray_popup_syncs_cached_snapshot_before_refreshing_when_shown", async () => {
  renderWithEnglish(<TrayPopup />);

  await waitFor(() => expect(mocks.refreshSnapshot).toHaveBeenCalledTimes(1));

  let resolveRefresh: ((snapshot: AppSnapshot) => void) | undefined;
  mocks.getCachedSnapshot.mockResolvedValueOnce({
    schemaVersion: 1,
    refreshedAt: "2026-06-08T10:05:00+08:00",
    providers: [
      {
        id: "cached-popup",
        name: "Cached Popup",
        status: "ok",
        source: "mock",
        updatedAt: "2026-06-08T10:05:00+08:00",
        error: null,
        diagnostics: null,
        metadata: null,
        windows: [
          {
            id: "cached-window",
            label: "Cached Window",
            used: 10,
            limit: 100,
            unit: "percent",
            usedPercent: 10,
            remainingPercent: 90,
            resetAt: null,
            resetText: "cached reset",
            confidence: "estimated",
          },
        ],
      },
    ],
  });
  mocks.refreshSnapshot.mockImplementationOnce(
    () =>
      new Promise<AppSnapshot>((resolve) => {
        resolveRefresh = resolve;
      }),
  );

  await act(async () => {
    mocks.listeners.trayShown?.(1);
  });

  await waitFor(() => expect(screen.getByText("Cached Popup")).toBeInTheDocument());
  expect(screen.getByText("Cached Window")).toBeInTheDocument();

  await act(async () => {
    resolveRefresh?.({
      schemaVersion: 1,
      refreshedAt: "2026-06-08T10:06:00+08:00",
      providers: [],
    });
  });
});

test("tray_popup_uses_native_snapshot_updates", async () => {
  renderWithEnglish(<TrayPopup />);

  await waitFor(() => expect(mocks.refreshSnapshot).toHaveBeenCalledTimes(1));

  await act(async () => {
    mocks.listeners.snapshotUpdated?.({
      schemaVersion: 1,
      refreshedAt: "2026-06-08T10:05:00+08:00",
      providers: [
        {
          id: "native-popup",
          name: "Native Popup",
          status: "ok",
          source: "mock",
          updatedAt: "2026-06-08T10:05:00+08:00",
          error: null,
          diagnostics: null,
          metadata: null,
          windows: [
            {
              id: "native-window",
              label: "Native Window",
              used: 20,
              limit: 100,
              unit: "percent",
              usedPercent: 20,
              remainingPercent: 80,
              resetAt: null,
              resetText: "native reset",
              confidence: "estimated",
            },
          ],
        },
      ],
    });
  });

  expect(screen.getByText("Native Popup")).toBeInTheDocument();
  expect(screen.getByText("Native Window")).toBeInTheDocument();
});

test("tray_popup_uses_window_focus_to_check_for_missed_presentation", async () => {
  renderWithEnglish(<TrayPopup />);

  await waitFor(() => expect(mocks.refreshSnapshot).toHaveBeenCalledTimes(1));

  mocks.state.presentationId = 1;
  fireEvent.focus(window);

  await waitFor(() => expect(mocks.refreshSnapshot).toHaveBeenCalledTimes(2));

  fireEvent.focus(window);
  expect(mocks.refreshSnapshot).toHaveBeenCalledTimes(2);

  await act(async () => {
    mocks.listeners.trayShown?.(2);
  });

  await waitFor(() => expect(mocks.refreshSnapshot).toHaveBeenCalledTimes(3));
});

test("tray_popup_groups_windows_by_provider_and_reset_stays_secondary", async () => {
  renderWithEnglish(<TrayPopup />);

  await waitFor(() => expect(mocks.refreshSnapshot).toHaveBeenCalled());

  expect(screen.getByText("Codex Mock")).toBeInTheDocument();
  expect(screen.getByText("Daily")).toBeInTheDocument();
  expect(screen.getByText("resets tomorrow")).toBeInTheDocument();
  expect(
    screen.queryByText("Codex Mock - resets tomorrow"),
  ).not.toBeInTheDocument();
});

test("tray_popup_colors_each_window_by_its_own_warning_status", async () => {
  renderWithEnglish(<TrayPopup />);

  await waitFor(() => expect(mocks.refreshSnapshot).toHaveBeenCalled());

  const normalFill = screen.getByRole("progressbar", {
    name: "Codex Mock Daily remaining",
  }).firstElementChild;
  const warningFill = screen.getByRole("progressbar", {
    name: "Codex Mock Extra window remaining",
  }).firstElementChild;

  expect(screen.getByText("Codex Mock Low")).toBeInTheDocument();
  expect(normalFill).toHaveClass("progress-fill--normal");
  expect(warningFill).toHaveClass("progress-fill--warning");
});

test("tray_popup_hides_popup_on_escape_and_close_button", async () => {
  renderWithEnglish(<TrayPopup />);

  fireEvent.keyDown(window, { key: "Escape" });
  fireEvent.click(screen.getByRole("button", { name: "Close" }));

  await waitFor(() => expect(mocks.hideTrayPopup).toHaveBeenCalledTimes(2));
  expect(mocks.hideCurrentWindow).not.toHaveBeenCalled();
});

test("tray_popup_follows_main_snapshot_provider_order_for_quota_windows", async () => {
  const { container } = renderWithEnglish(<TrayPopup />);

  await waitFor(() => expect(mocks.refreshSnapshot).toHaveBeenCalled());
  const popupText = container.textContent ?? "";

  expect(popupText.indexOf("Kimi")).toBeGreaterThanOrEqual(0);
  expect(popupText.indexOf("Codex Mock")).toBeGreaterThanOrEqual(0);
  expect(popupText.indexOf("Codex Mock")).toBeLessThan(popupText.indexOf("Kimi"));
});

test("tray_popup_shows_unhealthy_provider_in_title", async () => {
  mocks.refreshSnapshot.mockResolvedValueOnce({
    schemaVersion: 1,
    refreshedAt: "2026-06-08T10:00:00+08:00",
    providers: [
      {
        id: "remote-kimi",
        name: "Kimi",
        status: "error",
        source: "remote",
        updatedAt: null,
        error: "Refresh failed",
        diagnostics: null,
        metadata: null,
        windows: [],
      },
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
            confidence: "estimated",
          },
        ],
      },
    ],
  });

  renderWithEnglish(<TrayPopup />);

  await waitFor(() => expect(screen.getByText("Kimi Error")).toBeInTheDocument());
  expect(screen.queryByLabelText("Provider status")).not.toBeInTheDocument();
});

test("tray_popup_starts_native_dragging_from_titlebar", async () => {
  renderWithEnglish(<TrayPopup />);

  fireEvent.mouseDown(screen.getByTestId("tray-popup-titlebar"), { button: 2 });
  expect(mocks.startDraggingCurrentWindow).not.toHaveBeenCalled();

  fireEvent.mouseDown(screen.getByTestId("tray-popup-titlebar"), { button: 0 });

  await waitFor(() =>
    expect(mocks.startDraggingCurrentWindow).toHaveBeenCalledTimes(1),
  );
});

test("tray_popup_resets_size_from_titlebar_double_click", async () => {
  renderWithEnglish(<TrayPopup />);

  fireEvent.doubleClick(screen.getByTestId("tray-popup-titlebar"), { button: 0 });

  await waitFor(() =>
    expect(mocks.resetTrayPopupSize).toHaveBeenCalledTimes(1),
  );
});

test("tray_popup_starts_native_resizing_from_handle", async () => {
  renderWithEnglish(<TrayPopup />);

  fireEvent.mouseDown(screen.getByTestId("tray-popup-resize-handle"), { button: 2 });
  expect(mocks.startResizingCurrentWindow).not.toHaveBeenCalled();

  fireEvent.mouseDown(screen.getByTestId("tray-popup-resize-handle"), { button: 0 });

  await waitFor(() =>
    expect(mocks.startResizingCurrentWindow).toHaveBeenCalledTimes(1),
  );
});
