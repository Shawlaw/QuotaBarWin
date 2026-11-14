import { describe, expect, it } from "vitest";
import {
  applyVisibleWindowOrder,
  projectProviderSnapshot,
  projectSnapshotForConfig,
} from "./snapshotProjection";
import type {
  AppConfig,
  AppSnapshot,
  ProviderSnapshot,
  QuotaWindow,
  RemoteProviderConfig,
} from "../types";

function window(id: string, label = id): QuotaWindow {
  return {
    id,
    label,
    used: 10,
    limit: 100,
    unit: "percent",
    usedPercent: 10,
    remainingPercent: 90,
    resetAt: null,
    resetText: "later",
    confidence: "exact",
  };
}

function provider(id: string, windows: QuotaWindow[]): ProviderSnapshot {
  return {
    id,
    name: id,
    status: "ok",
    source: "remote",
    updatedAt: null,
    windows,
  };
}

function providerConfig(
  id: string,
  overrides: Partial<RemoteProviderConfig> = {},
): RemoteProviderConfig {
  return {
    id,
    name: id,
    enabled: true,
    kind: "remote",
    manifestUrl: `https://example.test/${id}/provider.json`,
    sourceUrl: `https://example.test/${id}/provider.cjs`,
    runtime: "node",
    autoUpdate: false,
    updateIntervalSeconds: 3600,
    timeoutSeconds: 30,
    ...overrides,
  };
}

function config(providers: RemoteProviderConfig[]): AppConfig {
  return {
    schemaVersion: 29,
    refreshIntervalSeconds: 300,
    displayMode: "remaining",
    lowQuotaWarningThreshold: 20,
    language: "en",
    remoteProviderRegistry: {
      registryUrl: null,
      providerProxyUrl: null,
      autoUpdate: true,
    },
    providers,
  };
}

describe("applyVisibleWindowOrder", () => {
  it("returns the original order when no windows are configured", () => {
    const windows = [window("daily"), window("weekly")];

    expect(applyVisibleWindowOrder(windows, [])).toEqual(windows);
    expect(applyVisibleWindowOrder(windows, undefined)).toEqual(windows);
  });

  it("reorders configured windows and hides the rest", () => {
    const windows = [window("daily"), window("weekly"), window("monthly")];

    expect(
      applyVisibleWindowOrder(windows, ["weekly", "daily"]).map((item) => item.id),
    ).toEqual(["weekly", "daily"]);
  });

  it("ignores blank and duplicate entries", () => {
    const windows = [window("daily"), window("weekly")];

    expect(
      applyVisibleWindowOrder(windows, ["weekly", " ", "weekly", "daily"]).map(
        (item) => item.id,
      ),
    ).toEqual(["weekly", "daily"]);
  });

  it("matches by label for legacy label-based configuration", () => {
    const windows = [window("daily", "Daily"), window("weekly", "Weekly")];

    expect(
      applyVisibleWindowOrder(windows, ["Weekly", "Daily"]).map((item) => item.id),
    ).toEqual(["weekly", "daily"]);
  });
});

describe("projectProviderSnapshot", () => {
  it("orders windows before applying label overrides", () => {
    const projected = projectProviderSnapshot(provider("remote-a", [window("daily"), window("weekly")]), {
      ...providerConfig("remote-a", { name: "Renamed" }),
      visibleWindowIds: ["weekly", "daily"],
      windowLabelOverrides: { weekly: "This week" },
    });

    expect(projected.name).toBe("Renamed");
    expect(projected.windows.map((item) => item.id)).toEqual(["weekly", "daily"]);
    expect(projected.windows[0].label).toBe("This week");
    expect(projected.windows[1].label).toBe("daily");
  });
});

describe("projectSnapshotForConfig", () => {
  it("follows provider order and drops disabled providers", () => {
    const snapshot: AppSnapshot = {
      schemaVersion: 1,
      refreshedAt: "2026-06-08T10:00:00+08:00",
      providers: [
        provider("remote-a", [window("daily")]),
        provider("remote-b", [window("weekly")]),
      ],
    };

    const projected = projectSnapshotForConfig(
      snapshot,
      config([
        providerConfig("remote-b"),
        providerConfig("remote-a", { enabled: false }),
      ]),
    );

    expect(projected?.providers.map((item) => item.id)).toEqual(["remote-b"]);
  });

  it("keeps a null snapshot null", () => {
    expect(projectSnapshotForConfig(null, config([]))).toBeNull();
  });
});
