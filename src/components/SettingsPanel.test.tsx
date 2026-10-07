import { useState } from "react";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, vi } from "vitest";
import { SettingsPanel } from "./SettingsPanel";
import { I18nProvider } from "../i18n";
import type {
  AppConfig,
  ConfigStorageInfo,
  LocalApiStatus,
  ProviderSnapshot,
  QuotaWindow,
  RemoteProviderConfig,
} from "../types";
import type { AppUpdateInfo } from "../lib/api";

const apiMocks = vi.hoisted(() => {
  const state: { config: AppConfig | null } = { config: null };
  return {
    state,
    applyAppUpdate: vi.fn(async () => undefined),
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
    checkAppUpdate: vi.fn(async () => ({
      configured: true,
      currentVersion: "1.0.3",
      available: true,
      version: "1.0.4",
      notesUrl: "https://example.com/releases/v1.0.4",
      downloaded: false,
    })),
    checkRemoteUpdates: vi.fn(async () => [
      {
        id: "remote-kimi",
        available: true,
        newChecksum: "sha256:new",
        updateManifestUrl: "https://registry.example.com/kimi/provider.json",
        currentVersion: "1.0.0",
        newVersion: "1.1.0",
        checkedAt: "2026-06-18T09:00:00Z",
      },
    ]),
    downloadAppUpdate: vi.fn(async () => ({
      configured: true,
      currentVersion: "1.0.3",
      available: true,
      version: "1.0.4",
      notesUrl: "https://example.com/releases/v1.0.4",
      downloaded: true,
    })),
    getAppUpdateStatus: vi.fn(async () => ({
      configured: true,
      currentVersion: "1.0.3",
      available: false,
      version: null,
      notesUrl: null,
      downloaded: false,
    })),
    getLocalApiAccessToken: vi.fn(async () => ({ token: "x".repeat(32) })),
    getLocalApiStatus: vi.fn<() => Promise<LocalApiStatus>>(
      () => new Promise<never>(() => undefined),
    ),
    getManagedSecretsEncryptionStatus: vi.fn<() => Promise<never>>(
      () => new Promise<never>(() => undefined),
    ),
    enableManagedSecretsEncryption: vi.fn(async () => 0),
    disableManagedSecretsEncryption: vi.fn(async () => 0),
    dismissManagedSecretsEncryptionPrompt: vi.fn(async () => undefined),
    getConfig: vi.fn(async () => state.config),
    devPreviewCloneSource: vi.fn(async () => ({
      sourceDir: "C:\Users\tester\AppData\Roaming\QuotaBarWin",
      fromPortableProcess: false,
    })),
    devCloneReleaseConfig: vi.fn(async () => ({
      sourceDir: "C:\Users\tester\AppData\Roaming\QuotaBarWin",
      fromPortableProcess: false,
    })),
    devRestartApp: vi.fn(async () => undefined),
    sendTestNotification: vi.fn(async () => ({
      toast: { status: "skipped", detail: "disabled" },
      webhook: { status: "skipped", detail: "disabled" },
    })),
    listenForAppUpdateStatus: vi.fn(async () => () => undefined),
    getInstalledRemoteProviderManifest: vi.fn(async (id: string) => ({
      schemaVersion: 1,
      id,
      displayName: "Remote Kimi",
      runtime: "node",
      entry: "provider.cjs",
      requiredEnvVars: ["KIMI_API_KEY"],
      output: "provider-snapshot-v1",
      parameters: [
        {
          name: "KIMI_API_KEY",
          label: "Kimi API Key",
          kind: "secret",
          required: true,
          defaultValue: "${secret:KIMI_API_KEY}",
          description: "Used to read Kimi coding quota."
        }
      ]
    })),
    getProviderSetup: vi.fn(async (providerId: string) => ({
      providerId,
      providerType: "catalog-kimi",
      displayName: "Catalog Kimi",
      setupState: "pending",
      fields: [],
      hasUnknownEnvVars: false,
      canAutoDetect: true,
    })),
    installRemoteProviderRegistry: vi.fn(async () => ({
      installed: [],
      skipped: [],
      failed: [],
    })),
    listLocalApiNetworkInterfaces: vi.fn(() => new Promise<never>(() => undefined)),
    installRemoteProviderManifest: vi.fn(async (url: string) => {
      const provider: RemoteProviderConfig = {
        id: "catalog-kimi",
        name: "Catalog Kimi",
        enabled: true,
        kind: "remote",
        version: "1.0.0",
        manifestUrl: url,
        sourceUrl: "https://example.com/catalog-kimi/provider.cjs",
        runtime: "node",
        autoUpdate: true,
        updateIntervalSeconds: 3600,
        timeoutSeconds: 30,
      };
      if (state.config) {
        state.config = {
          ...state.config,
          providers: [...state.config.providers, provider],
        };
      }
      return provider;
    }),
    migrateRemoteProvidersToRegistry: vi.fn(async () => ({
      migrated: ["remote-kimi"],
      skipped: [],
      failed: [],
    })),
    openAppUpdateNotes: vi.fn(async () => undefined),
    openRemoteProviderGuide: vi.fn(async () => undefined),
    previewRemoteProviderRegistry: vi.fn(async () => [
      {
        id: "catalog-kimi",
        displayName: "Catalog Kimi",
        version: "1.0.0",
        description: "Catalog provider",
        providerUrl: "https://example.com/catalog-kimi/provider.json",
        checksum: "sha256:manifest",
        installed: false,
        error: null,
      },
    ]),
    removeRemoteProvider: vi.fn(async (id: string) => {
      if (state.config) {
        state.config = {
          ...state.config,
          providers: state.config.providers.filter((provider) => provider.id !== id),
        };
      }
    }),
    saveProviderSetup: vi.fn(async () => undefined),
    setLocalApiAccessToken: vi.fn(async () => ({ token: "x".repeat(32) })),
    testProviderSetup: vi.fn(async () => ({ success: true, provider: null })),
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
  timeoutSeconds: 30,
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
    schemaVersion: 17,
    logMaxBytes: 10 * 1024 * 1024,
    logQuotaData: false,
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
  onSave: () => void | Promise<void> = () => undefined,
  navigation: {
    closeRequest?: number;
    appUpdateFocusRequest?: number;
    onAppUpdateFocusHandled?: () => void;
    onAppUpdateStatusChange?: (info: AppUpdateInfo) => void;
    initialProviderSettingsView?: "main" | "add";
    onRequestClose?: () => void;
  } = {},
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
          appVersion="1.0.5-test"
          config={config}
          configStorageInfo={storageInfo}
          isConfigStorageBusy={false}
          isSaving={false}
          onChange={handleChange}
          onOpenConfigFolder={async () => undefined}
          onResetConfig={async () => undefined}
          onSave={onSave}
          onSetPortableMode={() => undefined}
          onPersistedConfigChanged={handleChange}
          onRequestClose={navigation.onRequestClose ?? (() => undefined)}
          closeRequest={navigation.closeRequest ?? 0}
          settingsHomeRequest={0}
          appUpdateFocusRequest={navigation.appUpdateFocusRequest ?? 0}
          onAppUpdateFocusHandled={navigation.onAppUpdateFocusHandled ?? (() => undefined)}
          onAppUpdateStatusChange={navigation.onAppUpdateStatusChange ?? (() => undefined)}
          initialProviderSettingsView={navigation.initialProviderSettingsView ?? "main"}
          snapshotProviders={snapshotProviders}
        />
      </I18nProvider>
    );
  }

  return render(<Harness />);
}

// The settings page lands on the Providers category; controls in other
// categories need the left navigation clicked first, exactly like a user.
function switchSettingsCategory(category: string) {
  fireEvent.click(screen.getByTestId(`settings-nav-${category}`));
}

test("settings_renders_registry_and_remote_provider_metadata", async () => {
  renderSettings();

  expect(screen.getByRole("button", { name: "Add Provider" })).toBeInTheDocument();
  expect(screen.getByText("Remote Kimi")).toBeInTheDocument();
  expect(screen.getByText("1.0.0")).toBeInTheDocument();

  fireEvent.click(screen.getByTestId("edit-provider-remote-kimi"));

  expect(screen.getByText(/Version: 1.0.0/)).toBeInTheDocument();
  expect(screen.getByText(/Runtime: node/)).toBeInTheDocument();
  expect(screen.getByText(/Manifest:/)).toHaveTextContent("https://example.com/provider.json");
  expect(
    screen.getByText(/For multiple accounts, keep the left side as the script env var/)
  ).toBeInTheDocument();
  expect(await screen.findByText("Kimi API Key")).toBeInTheDocument();
  expect(screen.getByText(/Default: \$\{secret:KIMI_API_KEY\}/)).toBeInTheDocument();
});

test("settings_keeps_provider_metadata_and_actions_in_their_header_groups", () => {
  renderSettings(configWithProviders([{ ...remoteProvider, setupState: "unverified" }]));

  const provider = screen.getByTestId("settings-provider-remote-kimi");
  const metadata = provider.querySelector(".settings-provider__meta");
  const controls = provider.querySelector(".settings-provider__controls");

  expect(metadata).toHaveTextContent("1.0.0");
  expect(metadata).toHaveTextContent("Unverified");
  expect(controls).toContainElement(screen.getByTestId("setup-provider-remote-kimi"));
  expect(controls).toContainElement(screen.getByTestId("edit-provider-remote-kimi"));
  expect(controls).toContainElement(screen.getByTestId("more-provider-remote-kimi"));
});

test("settings_does_not_replay_a_close_request_when_the_provider_catalog_mounts", async () => {
  const onRequestClose = vi.fn();
  renderSettings(
    configWithProviders([]),
    [],
    configStorageInfo,
    () => undefined,
    {
      closeRequest: 1,
      initialProviderSettingsView: "add",
      onRequestClose,
    },
  );

  expect(await screen.findByTestId("add-provider-page")).toBeInTheDocument();
  await waitFor(() => expect(apiMocks.previewRemoteProviderRegistry).toHaveBeenCalled());
  expect(onRequestClose).not.toHaveBeenCalled();
});

test("settings_checks_and_applies_signed application updates", async () => {
  renderSettings();
  switchSettingsCategory("app-update");

  expect(screen.getByText("Current version: 1.0.5-test")).toBeInTheDocument();

  fireEvent.click(screen.getByRole("button", { name: "Check for updates" }));
  expect((await screen.findAllByText("QuotaBarWin 1.0.4 is available.")).length).toBeGreaterThan(0);

  fireEvent.click(screen.getByRole("button", { name: "Release notes" }));
  await waitFor(() =>
    expect(apiMocks.openAppUpdateNotes).toHaveBeenCalledWith(
      "https://example.com/releases/v1.0.4"
    )
  );

  fireEvent.click(screen.getByRole("button", { name: "Download and restart to update" }));
  await waitFor(() => expect(apiMocks.downloadAppUpdate).toHaveBeenCalledTimes(1));
  await waitFor(() => expect(apiMocks.applyAppUpdate).toHaveBeenCalledTimes(1));
  expect(apiMocks.checkAppUpdate).toHaveBeenCalledTimes(1);
});

test("manual application update checks notify the owning main window directly", async () => {
  const onAppUpdateStatusChange = vi.fn();
  renderSettings(configWithProviders([remoteProvider]), [], configStorageInfo, () => undefined, {
    onAppUpdateStatusChange,
  });
  switchSettingsCategory("app-update");

  fireEvent.click(screen.getByRole("button", { name: "Check for updates" }));

  await waitFor(() =>
    expect(onAppUpdateStatusChange).toHaveBeenCalledWith(
      expect.objectContaining({ available: true, version: "1.0.4" }),
    ),
  );
});

test("settings_saves_the_application_update_auto_check_toggle", async () => {
  renderSettings();
  switchSettingsCategory("app-update");

  const toggle = screen.getByTestId("app-update-auto-check") as HTMLInputElement;
  expect(toggle.checked).toBe(true);
  fireEvent.click(toggle);

  await waitFor(() => expect(apiMocks.state.config?.appUpdate?.autoCheck).toBe(false));
});

test("settings_toggles_provider_tray_visibility", async () => {
  renderSettings();

  fireEvent.click(screen.getByTestId("edit-provider-remote-kimi"));
  const trayToggle = await screen.findByTestId("provider-show-in-tray-remote-kimi");
  expect(trayToggle).toBeChecked();

  fireEvent.click(trayToggle);

  expect(apiMocks.state.config?.providers[0].showInTray).toBe(false);
});

test("settings_toggles_provider_notifications", async () => {
  renderSettings();

  fireEvent.click(screen.getByTestId("edit-provider-remote-kimi"));
  const notificationsToggle = await screen.findByTestId(
    "provider-notifications-enabled-remote-kimi",
  );
  expect(notificationsToggle).toBeChecked();

  fireEvent.click(notificationsToggle);

  expect(apiMocks.state.config?.providers[0].notificationsEnabled).toBe(false);
});

test("settings_edits_local_log_limit_in_megabytes", () => {
  renderSettings();
  switchSettingsCategory("advanced");

  const input = screen.getByTestId("log-max-size-input");
  expect(input).toHaveValue(10);

  fireEvent.change(input, { target: { value: "25" } });

  expect(apiMocks.state.config?.logMaxBytes).toBe(25 * 1024 * 1024);
});

test("settings_confirms_before_enabling_quota_data_logging", () => {
  const confirm = vi.spyOn(window, "confirm");
  renderSettings();
  switchSettingsCategory("advanced");

  const checkbox = screen.getByRole("checkbox", { name: "Log refreshed quota data" });
  fireEvent.click(checkbox);

  expect(confirm).not.toHaveBeenCalled();
  expect(screen.getByRole("dialog", { name: "Enable quota data logging?" })).toBeInTheDocument();
  expect(apiMocks.state.config?.logQuotaData).toBe(false);

  fireEvent.click(screen.getByRole("button", { name: "Cancel" }));

  expect(screen.queryByRole("dialog", { name: "Enable quota data logging?" })).not.toBeInTheDocument();
  expect(apiMocks.state.config?.logQuotaData).toBe(false);

  fireEvent.click(checkbox);
  fireEvent.click(screen.getByRole("button", { name: "Enable logging" }));

  expect(apiMocks.state.config?.logQuotaData).toBe(true);

  fireEvent.click(screen.getByRole("checkbox", { name: "Log refreshed quota data" }));

  expect(apiMocks.state.config?.logQuotaData).toBe(false);
  expect(confirm).not.toHaveBeenCalled();
});

test("settings_add_provider_page_installs_catalog_provider", async () => {
  renderSettings(configWithProviders([]));

  fireEvent.click(screen.getByRole("button", { name: "Add Provider" }));

  expect(screen.getByTestId("add-provider-page")).toBeInTheDocument();
  await screen.findByText("Catalog Kimi");
  fireEvent.click(screen.getByRole("button", { name: "Install" }));

  await waitFor(() =>
    expect(apiMocks.installRemoteProviderManifest).toHaveBeenCalledWith(
      "https://example.com/catalog-kimi/provider.json",
      "sha256:manifest",
      null,
      true,
    ),
  );

  expect(await screen.findByTestId("provider-setup-page")).toBeInTheDocument();
  fireEvent.click(await screen.findByRole("button", { name: "Back" }));
  expect(await screen.findByText("Catalog Kimi")).toBeInTheDocument();
});

test("saving a provider source returns to the catalog and refreshes it with its proxy", async () => {
  const onSave = vi.fn(async () => undefined);
  renderSettings(configWithProviders([]), [], configStorageInfo, onSave);

  fireEvent.click(screen.getByRole("button", { name: "Add Provider" }));
  await screen.findByText("Catalog Kimi");
  fireEvent.click(screen.getByRole("button", { name: "Manage Source" }));

  fireEvent.change(screen.getByLabelText("Installation source proxy URL (optional)"), {
    target: { value: "socks5://127.0.0.1:1080" },
  });
  expect(screen.getByLabelText("Installation source proxy URL (optional)")).toHaveAttribute(
    "placeholder",
    "http://host:port or socks5://host:port",
  );
  fireEvent.click(screen.getByTestId("save-settings-button"));

  await waitFor(() =>
    expect(onSave).toHaveBeenCalled(),
  );
  expect(await screen.findByTestId("add-provider-page")).toBeInTheDocument();
  await waitFor(() =>
    expect(apiMocks.previewRemoteProviderRegistry).toHaveBeenLastCalledWith(
      "https://raw.githubusercontent.com/Shawlaw/QuotaBarWin/main/examples/remote-providers/registry.json",
      "socks5://127.0.0.1:1080",
    ),
  );
});

test("saved message stays visible until the next edit", async () => {
  const onSave = vi.fn(async () => undefined);
  renderSettings(configWithProviders([remoteProvider]), [], configStorageInfo, onSave);
  switchSettingsCategory("general");

  fireEvent.change(screen.getByTestId("refresh-interval-input"), { target: { value: "120" } });
  fireEvent.click(screen.getByTestId("save-settings-button"));

  await waitFor(() => expect(onSave).toHaveBeenCalled());
  expect(await screen.findByText("Saved")).toBeInTheDocument();

  fireEvent.change(screen.getByTestId("refresh-interval-input"), { target: { value: "180" } });
  expect(screen.getByText("Unsaved changes")).toBeInTheDocument();
  expect(screen.queryByText("Saved")).not.toBeInTheDocument();
});

test("settings_edits_theme_preference", () => {
  renderSettings();
  switchSettingsCategory("general");

  const theme = screen.getByTestId("theme-select");
  expect(theme).toHaveValue("system");

  fireEvent.change(theme, { target: { value: "dark" } });

  expect(apiMocks.state.config?.theme).toBe("dark");
  expect(screen.getByTestId("save-settings-button")).toBeEnabled();
});

test("ctrl+s saves when there are unsaved changes", async () => {
  const onSave = vi.fn(async () => undefined);
  renderSettings(configWithProviders([remoteProvider]), [], configStorageInfo, onSave);
  switchSettingsCategory("general");

  expect(screen.getByTestId("save-settings-button")).toHaveAttribute("title", "Save (Ctrl+S)");

  fireEvent.keyDown(window, { key: "s", ctrlKey: true });
  expect(onSave).not.toHaveBeenCalled();

  fireEvent.change(screen.getByTestId("refresh-interval-input"), { target: { value: "120" } });
  fireEvent.keyDown(window, { key: "s", ctrlKey: true });

  await waitFor(() => expect(onSave).toHaveBeenCalledTimes(1));
  expect(await screen.findByText("Saved")).toBeInTheDocument();
});

test("settings_edits_remote_env_vars_and_window_display", async () => {
  renderSettings(configWithProviders([remoteProvider]), [
    providerSnapshot("remote-kimi", [
      quotaWindow("weekly", "Weekly"),
      quotaWindow("daily", "Daily"),
    ]),
  ]);

  fireEvent.click(screen.getByTestId("edit-provider-remote-kimi"));
  expect(await screen.findByText("Kimi API Key")).toBeInTheDocument();
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

test("settings_marks a ready provider with a refresh error as needing attention", () => {
  renderSettings(configWithProviders([{ ...remoteProvider, setupState: "ready" }]), [
    {
      ...providerSnapshot("remote-kimi", []),
      status: "error",
      error: "Request timed out",
    },
  ]);

  expect(screen.getByText("Needs attention")).toBeInTheDocument();
  expect(screen.getByTestId("setup-provider-remote-kimi")).toHaveTextContent("Repair configuration");
});

test("settings_edits_remote_provider_timeout", async () => {
  renderSettings();

  fireEvent.click(screen.getByTestId("edit-provider-remote-kimi"));
  expect(await screen.findByText("Kimi API Key")).toBeInTheDocument();
  const timeoutInput = screen.getByLabelText("Timeout (seconds)");
  expect(timeoutInput).toHaveValue(30);

  fireEvent.change(timeoutInput, { target: { value: "45" } });

  expect(apiMocks.state.config?.providers[0].timeoutSeconds).toBe(45);
});

test("settings_checks_and_applies_remote_provider_update", async () => {
  renderSettings();

  fireEvent.click(screen.getByRole("button", { name: "Check Updates" }));

  await screen.findByText("1 update(s) available");
  expect(apiMocks.checkRemoteUpdates).toHaveBeenCalledTimes(1);
  expect(screen.getByTestId("apply-provider-update-remote-kimi")).toBeInTheDocument();

  fireEvent.click(screen.getByTestId("apply-provider-update-remote-kimi"));

  await waitFor(() =>
    expect(apiMocks.applyRemoteUpdate).toHaveBeenCalledWith(
      "remote-kimi",
      "https://registry.example.com/kimi/provider.json"
    )
  );
  expect(await screen.findByText("1.1.0")).toBeInTheDocument();
});

test("settings_clears_stale_provider_updates_when_a_later_check_fails", async () => {
  renderSettings();

  fireEvent.click(screen.getByRole("button", { name: "Check Updates" }));
  expect(await screen.findByTestId("apply-provider-update-remote-kimi")).toBeInTheDocument();

  apiMocks.checkRemoteUpdates.mockRejectedValueOnce(new Error("Provider source is unavailable"));
  fireEvent.click(screen.getByRole("button", { name: "Check Updates" }));

  expect(await screen.findByText("Provider source is unavailable")).toBeInTheDocument();
  expect(screen.queryByTestId("apply-provider-update-remote-kimi")).not.toBeInTheDocument();
  expect(screen.queryByTestId("apply-all-provider-updates")).not.toBeInTheDocument();
});

test("settings_reorders_and_removes_remote_providers", async () => {
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
  expect(screen.getByRole("dialog", { name: "Unsaved changes" })).toBeInTheDocument();
  fireEvent.click(screen.getByTestId("discard-unsaved-changes"));

  expect(await screen.findByRole("dialog", { name: "Remove provider Remote DeepSeek?" })).toBeInTheDocument();
  expect(screen.getByTestId("remove-managed-secrets")).toBeChecked();
  fireEvent.click(screen.getByTestId("confirm-remove-provider"));

  await waitFor(() => expect(apiMocks.removeRemoteProvider).toHaveBeenCalled());
  expect(apiMocks.removeRemoteProvider).toHaveBeenCalledWith("remote-deepseek", true);
  expect(screen.queryByText("Remote DeepSeek")).not.toBeInTheDocument();
});

test("settings_save_bar_tracks_dirty_state_and_validation", () => {
  renderSettings();
  switchSettingsCategory("general");

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

test("settings blocks saving an external local API listener until a token is saved", async () => {
  apiMocks.getLocalApiStatus.mockResolvedValue({
    enabled: false,
    running: false,
    endpoints: [],
    requiresAuth: false,
    tokenConfigured: false,
    error: null,
  });
  renderSettings({
    ...configWithProviders([remoteProvider]),
    localApi: {
      enabled: true,
      bindTarget: { kind: "all-network-interfaces", includeLoopback: true },
      port: 41833,
    },
  });
  switchSettingsCategory("advanced");

  fireEvent.change(screen.getByTestId("local-api-port"), { target: { value: "41834" } });

  await waitFor(() => {
    expect(
      screen.getAllByText("Save an access token before saving network listener settings."),
    ).toHaveLength(2);
  });
  expect(screen.getByTestId("save-settings-button")).toBeDisabled();
});

test("settings_protects_unsaved_changes_before_opening_provider_catalog", async () => {
  const onSave = vi.fn(async () => undefined);
  renderSettings(configWithProviders([]), [], configStorageInfo, onSave);

  // Dirty the draft in General, then switch back to Providers: the draft
  // must survive the category switch and still gate provider navigation.
  switchSettingsCategory("general");
  fireEvent.change(screen.getByTestId("refresh-interval-input"), { target: { value: "120" } });
  switchSettingsCategory("providers");
  fireEvent.click(screen.getByRole("button", { name: "Add Provider" }));

  expect(screen.getByRole("dialog", { name: "Unsaved changes" })).toBeInTheDocument();
  expect(screen.queryByTestId("add-provider-page")).not.toBeInTheDocument();

  fireEvent.click(screen.getByTestId("cancel-unsaved-changes"));
  expect(screen.queryByRole("dialog", { name: "Unsaved changes" })).not.toBeInTheDocument();
  expect(screen.getByTestId("providers-settings-section")).toBeInTheDocument();

  fireEvent.click(screen.getByRole("button", { name: "Add Provider" }));
  fireEvent.click(screen.getByTestId("save-and-continue-unsaved-changes"));

  await waitFor(() => expect(onSave).toHaveBeenCalledTimes(1));
  expect(await screen.findByTestId("add-provider-page")).toBeInTheDocument();
});

test("settings_protects_unsaved_changes_before_provider_update_check", async () => {
  renderSettings();

  switchSettingsCategory("general");
  fireEvent.change(screen.getByTestId("refresh-interval-input"), { target: { value: "120" } });
  switchSettingsCategory("providers");
  fireEvent.click(screen.getByRole("button", { name: "Check Updates" }));

  expect(screen.getByRole("dialog", { name: "Unsaved changes" })).toBeInTheDocument();
  expect(apiMocks.checkRemoteUpdates).not.toHaveBeenCalled();

  fireEvent.click(screen.getByTestId("discard-unsaved-changes"));

  await waitFor(() => expect(apiMocks.checkRemoteUpdates).toHaveBeenCalledTimes(1));
});

test("settings_blocks_invalid_remote_provider_timeout", async () => {
  renderSettings();

  fireEvent.click(screen.getByTestId("edit-provider-remote-kimi"));
  expect(await screen.findByText("Kimi API Key")).toBeInTheDocument();
  fireEvent.change(screen.getByLabelText("Timeout (seconds)"), {
    target: { value: "0" },
  });

  expect(screen.getByText("Timeout must be greater than 0.")).toBeInTheDocument();
  expect(screen.getByTestId("save-settings-button")).toBeDisabled();
});

test("settings_opens_on_the_providers_category_with_category_navigation", () => {
  renderSettings();

  expect(screen.getByTestId("providers-settings-section")).toBeInTheDocument();
  expect(screen.queryByTestId("general-settings-section")).not.toBeInTheDocument();
  expect(screen.queryByTestId("notification-settings-section")).not.toBeInTheDocument();
  expect(screen.queryByTestId("app-update-section")).not.toBeInTheDocument();
  expect(screen.queryByTestId("advanced-settings-section")).not.toBeInTheDocument();

  const navigation = screen.getByTestId("settings-nav");
  for (const label of ["Providers", "General", "Notifications", "Application update", "Advanced"]) {
    expect(navigation).toHaveTextContent(label);
  }
  // The guide link lives on the rail below the category tabs and opens the
  // same exported guide as the provider catalog entry.
  expect(screen.getByTestId("settings-guide-link")).toHaveTextContent("Open guide");
  expect(apiMocks.openRemoteProviderGuide).not.toHaveBeenCalled();
  fireEvent.click(screen.getByTestId("settings-guide-link"));
  expect(apiMocks.openRemoteProviderGuide).toHaveBeenCalledTimes(1);
});

test("switching_categories_keeps_the_unsaved_draft", () => {
  renderSettings();

  switchSettingsCategory("general");
  fireEvent.change(screen.getByTestId("refresh-interval-input"), { target: { value: "120" } });
  switchSettingsCategory("notifications");
  expect(screen.getByTestId("notification-settings-section")).toBeInTheDocument();

  switchSettingsCategory("general");
  expect(screen.getByTestId("refresh-interval-input")).toHaveValue(120);
  expect(screen.getByTestId("fixed-save-bar")).toHaveTextContent("Unsaved changes");
});

test("general_low_quota_summary_links_to_the_notification_editor", () => {
  renderSettings();

  switchSettingsCategory("general");
  expect(screen.getByTestId("low-quota-summary")).toHaveTextContent("Current 20%");
  expect(screen.queryByTestId("low-quota-warning-input")).not.toBeInTheDocument();

  fireEvent.click(screen.getByRole("button", { name: "Open notification settings" }));

  expect(screen.getByTestId("notification-settings-section")).toBeInTheDocument();
  expect(screen.getByTestId("low-quota-warning-input")).toHaveValue(20);
});

test("notifications_category_edits_the_shared_low_quota_threshold", () => {
  renderSettings();

  switchSettingsCategory("notifications");
  fireEvent.change(screen.getByTestId("low-quota-warning-input"), { target: { value: "35" } });

  expect(apiMocks.state.config?.lowQuotaWarningThreshold).toBe(35);
  expect(screen.getByTestId("fixed-save-bar")).toHaveTextContent("Unsaved changes");
});

function ResetEpochHarness() {
  const [config, setConfig] = useState(configWithProviders([remoteProvider]));
  const [epoch, setEpoch] = useState(0);
  const reset = async () => {
    // Mirrors App: the backend resets the file and pushes the defaults.
    const defaults = { ...configWithProviders([]), refreshIntervalSeconds: 300 };
    setConfig(defaults);
    setEpoch((current) => current + 1);
  };
  return (
    <I18nProvider language="en">
      <SettingsPanel
        appVersion="1.0.5-test"
        persistedConfigEpoch={epoch}
        config={config}
        configStorageInfo={configStorageInfo}
        isConfigStorageBusy={false}
        isSaving={false}
        onChange={setConfig}
        onOpenConfigFolder={async () => undefined}
        onResetConfig={reset}
        onSave={() => undefined}
        onSetPortableMode={() => undefined}
        onPersistedConfigChanged={() => undefined}
        onRequestClose={() => undefined}
        closeRequest={0}
        settingsHomeRequest={0}
        appUpdateFocusRequest={0}
        onAppUpdateFocusHandled={() => undefined}
        onAppUpdateStatusChange={() => undefined}
        initialProviderSettingsView="main"
      />
    </I18nProvider>
  );
}

test("resetting_config_to_defaults_does_not_require_a_manual_save", async () => {
  render(<ResetEpochHarness />);
  switchSettingsCategory("advanced");

  fireEvent.click(screen.getByRole("button", { name: "Reset config" }));
  fireEvent.click(screen.getByTestId("confirm-reset-config"));

  // The draft is rebased onto the persisted defaults: no unsaved state.
  await waitFor(() => {
    expect(screen.getByTestId("fixed-save-bar")).toHaveTextContent("Saved");
  });
  expect(screen.getByTestId("save-settings-button")).toBeDisabled();
});

test("copying_a_storage_path_shows_a_copied_hint", async () => {
  const writeText = vi.fn(async () => undefined);
  Object.defineProperty(window.navigator, "clipboard", {
    configurable: true,
    value: { writeText },
  });

  renderSettings();
  switchSettingsCategory("advanced");

  expect(screen.queryByTestId("path-copied-hint")).not.toBeInTheDocument();
  fireEvent.click(screen.getByTitle(configStorageInfo.configPath));

  expect(writeText).toHaveBeenCalledWith(configStorageInfo.configPath);
  // The hint appears after the clipboard promise resolves.
  await waitFor(() => expect(screen.getByTestId("path-copied-hint")).toBeInTheDocument());
  expect(screen.getByTestId("path-copied-hint")).toHaveTextContent("Path copied to the clipboard.");
});

test("dev_clone_shows_the_resolved_source_before_copying", async () => {
  apiMocks.state.config = configWithProviders([remoteProvider]);
  render(
    <I18nProvider language="en">
      <SettingsPanel
        appVersion="1.0.5-test"
        isDevBuild
        config={configWithProviders([remoteProvider])}
        configStorageInfo={configStorageInfo}
        isConfigStorageBusy={false}
        isSaving={false}
        onChange={() => undefined}
        onOpenConfigFolder={async () => undefined}
        onResetConfig={async () => undefined}
        onSave={() => undefined}
        onSetPortableMode={() => undefined}
        onPersistedConfigChanged={() => undefined}
        onRequestClose={() => undefined}
        closeRequest={0}
        settingsHomeRequest={0}
        appUpdateFocusRequest={0}
        onAppUpdateFocusHandled={() => undefined}
        onAppUpdateStatusChange={() => undefined}
        initialProviderSettingsView="main"
      />
    </I18nProvider>,
  );

  switchSettingsCategory("advanced");
  fireEvent.click(screen.getByTestId("dev-clone-config-button"));

  // The confirm dialog resolves and shows the exact source first.
  const dialog = await screen.findByRole("dialog", { name: "Copy release config" });
  expect(dialog).toBeInTheDocument();
  expect(screen.getByTestId("dev-clone-source")).toHaveTextContent("AppData installation");
  expect(apiMocks.devCloneReleaseConfig).not.toHaveBeenCalled();

  fireEvent.click(screen.getByTestId("confirm-dev-clone"));
  await waitFor(() => expect(apiMocks.devCloneReleaseConfig).toHaveBeenCalledTimes(1));
});

test("dev_builds_lock_autostart_and_app_update_controls", () => {
  apiMocks.state.config = configWithProviders([remoteProvider]);
  render(
    <I18nProvider language="en">
      <SettingsPanel
        appVersion="1.0.5-test"
        isDevBuild
        config={configWithProviders([remoteProvider])}
        configStorageInfo={configStorageInfo}
        isConfigStorageBusy={false}
        isSaving={false}
        onChange={() => undefined}
        onOpenConfigFolder={async () => undefined}
        onResetConfig={async () => undefined}
        onSave={() => undefined}
        onSetPortableMode={() => undefined}
        onPersistedConfigChanged={() => undefined}
        onRequestClose={() => undefined}
        closeRequest={0}
        settingsHomeRequest={0}
        appUpdateFocusRequest={0}
        onAppUpdateFocusHandled={() => undefined}
        onAppUpdateStatusChange={() => undefined}
        initialProviderSettingsView="main"
      />
    </I18nProvider>,
  );

  switchSettingsCategory("general");
  const startup = screen.getByTestId("launch-at-startup-toggle");
  expect(startup).toBeDisabled();
  expect(screen.getByText("Not available in development builds.")).toBeInTheDocument();

  switchSettingsCategory("app-update");
  expect(screen.getByTestId("app-update-auto-check")).toBeDisabled();
  expect(screen.getByTestId("app-update-dev-hint")).toBeInTheDocument();
  expect(screen.getByRole("button", { name: "Check for updates" })).toBeDisabled();
});
