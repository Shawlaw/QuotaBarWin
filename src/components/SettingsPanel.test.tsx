import { useState } from "react";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, vi } from "vitest";
import { SettingsPanel } from "./SettingsPanel";
import { I18nProvider } from "../i18n";
import type {
  AppConfig,
  ConfigStorageInfo,
  ProviderSnapshot,
  QuotaWindow,
  RemoteProviderConfig,
} from "../types";

const apiMocks = vi.hoisted(() => {
  const state: { config: AppConfig | null } = { config: null };
  return {
    state,
    applyRemoteUpdate: vi.fn(async (id: string) => {
      if (state.config) {
        state.config = {
          ...state.config,
          providers: state.config.providers.map((provider) =>
            provider.id === id
              ? { ...provider, version: "1.1.0", updatedAt: "2026-06-18T09:30:00Z" }
              : provider
          ),
        };
      }
    }),
    checkRemoteUpdates: vi.fn(async () => [
      {
        id: "remote-kimi",
        available: true,
        newChecksum: "sha256:new",
        currentVersion: "1.0.0",
        newVersion: "1.1.0",
        checkedAt: "2026-06-18T09:00:00Z",
      },
    ]),
    getConfig: vi.fn(async () => state.config),
    installRemoteProviderRegistry: vi.fn(async () => ({
      installed: [],
      skipped: [],
      failed: [],
    })),
    openRemoteProviderGuide: vi.fn(async () => undefined),
    refreshRemoteProvider: vi.fn(async () => ({
      id: "remote-kimi",
      available: true,
      newChecksum: "sha256:new",
      currentVersion: "1.0.0",
      newVersion: "1.1.0",
      checkedAt: "2026-06-18T09:00:00Z",
    })),
    removeRemoteProvider: vi.fn(async (id: string) => {
      if (state.config) {
        state.config = {
          ...state.config,
          providers: state.config.providers.filter((provider) => provider.id !== id),
        };
      }
    }),
  };
});

vi.mock("../lib/api", () => apiMocks);

afterEach(() => {
  vi.restoreAllMocks();
  apiMocks.state.config = null;
});

const remoteProvider: RemoteProviderConfig = {
  id: "remote-kimi",
  name: "Remote Kimi",
  enabled: true,
  kind: "remote",
  version: "1.0.0",
  manifestUrl: "https://example.com/provider.json",
  sourceUrl: "https://example.com/provider.cjs",
  providerDir: "C:\\Users\\tester\\AppData\\Roaming\\QuotaBarWin\\providers\\remote\\remote-kimi",
  runtime: "node",
  resolvedRuntime: "C:\\Program Files\\nodejs\\node.exe",
  proxyUrl: null,
  autoUpdate: true,
  updateIntervalSeconds: 3600,
  trustedChecksum: "sha256:abcdef1234567890",
  installedAt: "2026-06-18T08:00:00Z",
  updatedAt: "2026-06-18T08:10:00Z",
  lastCheckedAt: "2026-06-18T08:20:00Z",
  windowLabelOverrides: {
    weekly: "Weekly limit",
  },
  visibleWindowIds: ["weekly"],
  envVars: {
    KIMI_API_KEY: "${secret:KIMI_API_KEY}",
  },
};

const configStorageInfo: ConfigStorageInfo = {
  mode: "app-data",
  configPath:
    "C:\\Users\\tester\\AppData\\Roaming\\QuotaBarWin\\config.quotaBarWin.json",
  configDir: "C:\\Users\\tester\\AppData\\Roaming\\QuotaBarWin",
  appDataConfigPath:
    "C:\\Users\\tester\\AppData\\Roaming\\QuotaBarWin\\config.quotaBarWin.json",
  portableConfigPath: "C:\\Tools\\QuotaBarWin\\config.quotaBarWin.json",
  portableMarkerPath: "C:\\Tools\\QuotaBarWin\\quotabarwin.portable",
};

function configWithProviders(providers: AppConfig["providers"]): AppConfig {
  return {
    schemaVersion: 11,
    refreshIntervalSeconds: 300,
    displayMode: "remaining",
    lowQuotaWarningThreshold: 20,
    language: "system",
    networkProxy: null,
    remoteProviderRegistry: {
      registryUrl: null,
      providerProxyUrl: null,
      autoUpdate: true,
    },
    providers,
  };
}

function quotaWindow(id: string, label: string): QuotaWindow {
  return {
    id,
    label,
    used: null,
    limit: null,
    usedPercent: null,
    remainingPercent: null,
    resetAt: null,
    confidence: "unknown",
  };
}

function providerSnapshot(
  id: string,
  windows: QuotaWindow[],
): ProviderSnapshot {
  return {
    id,
    name: id,
    status: "ok",
    source: "remote",
    updatedAt: "2026-06-14T00:00:00Z",
    windows,
  };
}

function renderSettings(
  initialConfig = configWithProviders([remoteProvider]),
  snapshotProviders: ProviderSnapshot[] = [],
  storageInfo: ConfigStorageInfo | null = configStorageInfo,
) {
  apiMocks.state.config = initialConfig;

  function Harness() {
    const [config, setConfig] = useState(initialConfig);
    function handleChange(next: AppConfig) {
      apiMocks.state.config = next;
      setConfig(next);
    }
    return (
      <I18nProvider language="en">
        <SettingsPanel
          config={config}
          configStorageInfo={storageInfo}
          isConfigStorageBusy={false}
          isSaving={false}
          onChange={handleChange}
          onOpenConfigFolder={async () => undefined}
          onResetConfig={async () => undefined}
          onSave={() => undefined}
          onSetPortableMode={() => undefined}
          snapshotProviders={snapshotProviders}
        />
      </I18nProvider>
    );
  }

  return render(<Harness />);
}

test("settings_renders_registry_and_remote_provider_metadata", () => {
  renderSettings();

  expect(screen.getByTestId("remote-providers-section")).toBeInTheDocument();
  expect(screen.getByText("Remote Kimi")).toBeInTheDocument();
  expect(screen.getByText("1.0.0")).toBeInTheDocument();

  fireEvent.click(screen.getByTestId("edit-provider-remote-kimi"));

  expect(screen.getByText(/Version: 1.0.0/)).toBeInTheDocument();
  expect(screen.getByText(/Runtime: node/)).toBeInTheDocument();
  expect(screen.getByText(/Manifest:/)).toHaveTextContent("https://example.com/provider.json");
});

test("settings_edits_remote_env_vars_and_window_display", () => {
  renderSettings(configWithProviders([remoteProvider]), [
    providerSnapshot("remote-kimi", [
      quotaWindow("weekly", "Weekly"),
      quotaWindow("daily", "Daily"),
    ]),
  ]);

  fireEvent.click(screen.getByTestId("edit-provider-remote-kimi"));
  fireEvent.change(screen.getByLabelText("Environment variables"), {
    target: { value: "KIMI_API_KEY=${secret:TEAM_KIMI_API_KEY}" },
  });
  fireEvent.click(screen.getByLabelText("Show daily"));
  fireEvent.change(screen.getByLabelText("Custom label for daily"), {
    target: { value: "Team daily" },
  });

  expect(screen.getByLabelText("Environment variables")).toHaveValue(
    "KIMI_API_KEY=${secret:TEAM_KIMI_API_KEY}",
  );
  expect(screen.getByLabelText("Show daily")).toBeChecked();
  expect(screen.getByLabelText("Custom label for daily")).toHaveValue("Team daily");
});

test("settings_checks_and_applies_remote_provider_update", async () => {
  renderSettings();

  fireEvent.click(screen.getByRole("button", { name: "More" }));
  fireEvent.click(screen.getAllByRole("button", { name: "Check Updates" }).at(-1)!);

  await screen.findByText("Update available");
  expect(apiMocks.refreshRemoteProvider).toHaveBeenCalledWith("remote-kimi");

  fireEvent.click(screen.getByRole("button", { name: "Apply Update" }));

  await waitFor(() => expect(apiMocks.applyRemoteUpdate).toHaveBeenCalledWith("remote-kimi"));
  expect(await screen.findByText("1.1.0")).toBeInTheDocument();
});

test("settings_reorders_and_removes_remote_providers", async () => {
  vi.spyOn(window, "confirm").mockReturnValue(true);
  renderSettings(
    configWithProviders([
      remoteProvider,
      { ...remoteProvider, id: "remote-deepseek", name: "Remote DeepSeek" },
    ]),
  );

  fireEvent.click(screen.getAllByRole("button", { name: "More" })[1]);
  fireEvent.click(screen.getByRole("button", { name: "Up" }));
  expect(screen.getByText("Remote DeepSeek")).toBeInTheDocument();

  fireEvent.click(screen.getAllByRole("button", { name: "Remove" })[0]);

  await waitFor(() => expect(apiMocks.removeRemoteProvider).toHaveBeenCalled());
  expect(screen.queryByText("Remote DeepSeek")).not.toBeInTheDocument();
});

test("settings_save_bar_tracks_dirty_state_and_validation", () => {
  renderSettings();

  expect(screen.getByTestId("fixed-save-bar")).toHaveTextContent("No changes");
  expect(screen.getByTestId("save-settings-button")).toBeDisabled();

  fireEvent.change(
    screen.getByRole("spinbutton", { name: /Refresh interval/ }),
    {
      target: { value: "0" },
    },
  );

  expect(
    screen.getByText("Refresh interval must be greater than 0."),
  ).toBeInTheDocument();
  expect(screen.getByTestId("save-settings-button")).toBeDisabled();
});
