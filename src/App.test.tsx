import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { App } from "./App";
import type { AppConfig, AppSnapshot } from "./types";

const mocks = vi.hoisted(() => {
  const config: AppConfig = {
    schemaVersion: 1,
    refreshIntervalSeconds: 300,
    displayMode: "remaining",
    lowQuotaWarningThreshold: 20,
    providers: [
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
        windows: []
      }
    ]
  };

  return {
    getCachedSnapshot: vi.fn(async () => null),
    getConfig: vi.fn(async () => config),
    getProviderPresets: vi.fn(async () => []),
    listenForRefreshRequests: vi.fn(async () => () => undefined),
    refreshSnapshot: vi.fn(async () => snapshot),
    saveConfig: vi.fn(async () => undefined),
    testProvider: vi.fn(async () => snapshot.providers[0])
  };
});

vi.mock("./lib/api", () => mocks);

test("refresh_button_calls_refresh_snapshot", async () => {
  render(<App />);

  await waitFor(() => expect(mocks.refreshSnapshot).toHaveBeenCalledTimes(1));
  fireEvent.click(screen.getByRole("button", { name: "Refresh" }));

  await waitFor(() => expect(mocks.refreshSnapshot).toHaveBeenCalledTimes(2));
});
