import { useState } from "react";
import { fireEvent, render, screen } from "@testing-library/react";
import { afterEach, vi } from "vitest";
import { SettingsPanel } from "./SettingsPanel";
import { I18nProvider } from "../i18n";
import type {
  AppConfig,
  ConfigStorageInfo,
  ProviderPreset,
  ProviderSnapshot,
  QuotaWindow,
} from "../types";

afterEach(() => {
  vi.restoreAllMocks();
});

const codexProvider = {
  id: "codex",
  name: "Codex",
  enabled: true,
  kind: "codex" as const,
  authToken: "${env:CODEX_ACCESS_TOKEN}",
  accountId: null,
  proxyUrl: null,
  timeoutMs: 15000,
  windowLabelOverrides: {
    "5h": "5h",
    weekly: "Weekly limit",
  },
  visibleWindowIds: ["5h", "Weekly limit"],
};

const mockProvider = {
  id: "mock-codex",
  name: "Codex Mock",
  enabled: true,
  kind: "mock" as const,
};

const presets: ProviderPreset[] = [
  {
    id: "codex-usage",
    displayName: "Codex Usage",
    description: "Codex usage",
    requiredEnvVars: ["CODEX_ACCESS_TOKEN"],
    providerConfigTemplate: codexProvider,
  },
];

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
    schemaVersion: 10,
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
    source: "native",
    updatedAt: "2026-06-14T00:00:00Z",
    windows,
  };
}

function renderSettings(
  initialConfig = configWithProviders([codexProvider]),
  snapshotProviders: ProviderSnapshot[] = [],
  storageInfo: ConfigStorageInfo | null = configStorageInfo,
) {
  function Harness() {
    const [config, setConfig] = useState(initialConfig);
    return (
      <I18nProvider language="en">
        <SettingsPanel
          config={config}
          configStorageInfo={storageInfo}
          isConfigStorageBusy={false}
          isSaving={false}
          onChange={setConfig}
          onOpenConfigFolder={async () => undefined}
          onResetConfig={async () => undefined}
          onSave={() => undefined}
          onSetPortableMode={() => undefined}
          presets={presets}
          snapshotProviders={snapshotProviders}
        />
      </I18nProvider>
    );
  }

  return render(<Harness />);
}

test("settings_can_render_codex_provider", () => {
  renderSettings();

  expect(screen.getByLabelText("Refresh interval (seconds)")).toHaveValue(300);
  fireEvent.click(screen.getByRole("button", { name: "Edit" }));
  expect(screen.getByDisplayValue("Codex")).toBeInTheDocument();
  expect(screen.getByLabelText("Auth token")).toHaveValue(
    "${env:CODEX_ACCESS_TOKEN}",
  );
  expect(screen.getByLabelText("ChatGPT account id")).toHaveValue("");
  expect(screen.getByLabelText("Proxy URL")).toHaveValue("");
  expect(
    screen.getByRole("heading", { name: "Window display" }),
  ).toBeInTheDocument();
  expect(
    screen.getByText("No recent snapshot windows yet."),
  ).toBeInTheDocument();
  expect(screen.getByLabelText("Custom label for weekly")).toHaveValue(
    "Weekly limit",
  );
});

test("settings_edits_window_label_overrides", () => {
  renderSettings();

  fireEvent.click(screen.getByRole("button", { name: "Edit" }));
  const override = screen.getByLabelText("Custom label for weekly");
  fireEvent.change(override, {
    target: { value: "Team weekly limit" },
  });

  expect(screen.getByLabelText("Custom label for weekly")).toHaveValue(
    "Team weekly limit",
  );
});

test("settings_keeps_label_override_newlines_while_editing", () => {
  renderSettings();

  fireEvent.click(screen.getByRole("button", { name: "Edit" }));
  fireEvent.click(screen.getByText("Advanced window text"));
  const overrides = screen.getByLabelText("Window label overrides");
  fireEvent.change(overrides, {
    target: { value: "weekly=Team weekly limit\n" },
  });

  expect(screen.getByLabelText("Window label overrides")).toHaveValue(
    "weekly=Team weekly limit\n",
  );
});

test("settings_shows_add_provider_without_local_script_presets", () => {
  renderSettings();

  expect(
    screen.getByRole("heading", { name: "Add Provider" }),
  ).toBeInTheDocument();
  expect(
    screen.getByRole("button", { name: "Codex Usage" }),
  ).toBeInTheDocument();
  expect(
    screen.queryByRole("button", { name: "Kimi Coding Usage" }),
  ).not.toBeInTheDocument();
  expect(
    screen.queryByRole("button", { name: "OpenCode Quota Command" }),
  ).not.toBeInTheDocument();
  expect(
    screen.queryByRole("button", { name: "Custom Script Provider" }),
  ).not.toBeInTheDocument();
});

test("add_codex_preset_shows_token_fields", () => {
  renderSettings(configWithProviders([]));

  fireEvent.click(screen.getByRole("button", { name: "Codex Usage" }));

  expect(screen.getByText("Set CODEX_ACCESS_TOKEN")).toBeInTheDocument();
  expect(screen.getByLabelText("Auth token")).toHaveValue(
    "${env:CODEX_ACCESS_TOKEN}",
  );
  expect(
    screen.getByRole("heading", { name: "Window display" }),
  ).toBeInTheDocument();
  expect(screen.getByLabelText("Custom label for weekly")).toHaveValue(
    "Weekly limit",
  );
});

test("codex_provider_proxy_url_is_editable", () => {
  renderSettings(configWithProviders([]));

  fireEvent.click(screen.getByRole("button", { name: "Codex Usage" }));
  fireEvent.change(screen.getByLabelText("Proxy URL"), {
    target: { value: "socks5h://127.0.0.1:7890" },
  });

  expect(screen.getByLabelText("Proxy URL")).toHaveValue(
    "socks5h://127.0.0.1:7890",
  );
});

test("settings_shows_config_storage_info_and_remote_guide_entry", () => {
  renderSettings();

  expect(screen.getByTestId("general-settings-section")).toBeInTheDocument();
  expect(screen.getByTestId("providers-settings-section")).toBeInTheDocument();
  expect(screen.queryByTestId("advanced-settings-section")).not.toBeInTheDocument();
  expect(screen.getByLabelText("Configuration storage")).toBeInTheDocument();
  expect(
    screen.getByRole("button", { name: configStorageInfo.configPath }),
  ).toBeInTheDocument();
  expect(screen.getByText("AppData mode")).toBeInTheDocument();
  expect(screen.queryByText("Portable")).not.toBeInTheDocument();
  expect(
    screen.queryByText(configStorageInfo.portableConfigPath),
  ).not.toBeInTheDocument();
  expect(
    screen.queryByRole("heading", { name: "Custom Provider Guide" }),
  ).not.toBeInTheDocument();
  expect(
    screen.getByRole("heading", { name: "Remote Sources" }),
  ).toBeInTheDocument();
  expect(
    screen.getByRole("button", { name: "Open Guide" }),
  ).toBeInTheDocument();
});

test("settings_hides_app_data_path_in_portable_storage_mode", () => {
  const portableStorageInfo: ConfigStorageInfo = {
    ...configStorageInfo,
    mode: "portable",
    configPath: configStorageInfo.portableConfigPath,
    configDir: "C:\\Tools\\QuotaBarWin",
  };

  renderSettings(configWithProviders([codexProvider]), [], portableStorageInfo);

  expect(screen.getAllByText("Portable mode")).toHaveLength(2);
  expect(screen.getByText("Marker file")).toBeInTheDocument();
  expect(
    screen.getByRole("button", { name: portableStorageInfo.configPath }),
  ).toBeInTheDocument();
  expect(
    screen.getByText(portableStorageInfo.portableMarkerPath),
  ).toBeInTheDocument();
  expect(
    screen.queryByText(portableStorageInfo.appDataConfigPath),
  ).not.toBeInTheDocument();
});

test("settings_edits_visible_windows", () => {
  renderSettings({
    ...configWithProviders([codexProvider]),
    providers: [
      {
        ...codexProvider,
        visibleWindowIds: ["weekly"],
      },
    ],
  });

  fireEvent.click(screen.getByRole("button", { name: "Edit" }));
  expect(screen.getByLabelText("Show weekly")).toBeChecked();
  expect(screen.getByLabelText("Show 5h")).not.toBeChecked();

  fireEvent.click(screen.getByLabelText("Show 5h"));
  fireEvent.click(screen.getByText("Advanced window text"));

  expect(screen.getByLabelText("Displayed windows")).toHaveValue("weekly\n5h");
});

test("settings_uses_snapshot_windows_for_visual_window_display", () => {
  renderSettings(
    configWithProviders([
      { ...codexProvider, visibleWindowIds: ["5h", "weekly"] },
    ]),
    [
      providerSnapshot("codex", [
        quotaWindow("5h", "5h window"),
        quotaWindow("weekly", "Weekly limit"),
      ]),
    ],
  );

  fireEvent.click(screen.getByRole("button", { name: "Edit" }));

  expect(screen.getByText("5h window")).toBeInTheDocument();
  expect(screen.getByText("weekly")).toBeInTheDocument();
  fireEvent.click(screen.getByLabelText("Move weekly up"));
  fireEvent.click(screen.getByText("Advanced window text"));

  expect(screen.getByLabelText("Displayed windows")).toHaveValue("weekly\n5h");
});

test("settings_reorders_providers", () => {
  renderSettings(
    configWithProviders([
      codexProvider,
      {
        ...codexProvider,
        id: "codex-2",
        name: "Second Codex",
      },
    ]),
  );

  fireEvent.click(screen.getAllByRole("button", { name: "More" })[1]);
  fireEvent.click(screen.getByRole("button", { name: "Up" }));

  expect(screen.getByText("Second Codex")).toBeInTheDocument();
});

test("remove_provider_deletes_provider_from_settings", () => {
  vi.spyOn(window, "confirm").mockReturnValue(true);
  renderSettings();

  fireEvent.click(screen.getByRole("button", { name: "More" }));
  fireEvent.click(screen.getByRole("button", { name: "Remove" }));

  expect(screen.queryByText("Codex")).not.toBeInTheDocument();
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

  fireEvent.change(
    screen.getByRole("spinbutton", { name: /Refresh interval/ }),
    {
      target: { value: "120" },
    },
  );

  expect(screen.getByTestId("fixed-save-bar")).toHaveTextContent(
    "Unsaved changes",
  );
  expect(screen.getByTestId("save-settings-button")).toBeEnabled();
});

test("network_proxy_section_renders_and_switches_proxy_kind", () => {
  renderSettings(configWithProviders([mockProvider]));

  expect(screen.getByTestId("proxy-kind-select")).toBeInTheDocument();

  fireEvent.change(screen.getByTestId("proxy-kind-select"), {
    target: { value: "http" },
  });

  expect(screen.getByTestId("proxy-url-input")).toBeInTheDocument();
});

test("settings_edits_language_choice", () => {
  renderSettings(configWithProviders([mockProvider]));

  fireEvent.change(screen.getByTestId("language-select"), {
    target: { value: "zh-CN" },
  });

  expect(screen.getByTestId("language-select")).toHaveValue("zh-CN");
  expect(screen.getByTestId("fixed-save-bar")).toHaveTextContent(
    "Unsaved changes",
  );
});

test("remote_providers_section_renders_installed_remote_providers", () => {
  const remoteProvider = {
    id: "remote-kimi",
    name: "Remote Kimi",
    enabled: true,
    kind: "remote" as const,
    manifestUrl: "https://example.com/provider.json",
    sourceUrl: "https://example.com/provider.cjs",
    runtime: "node",
    autoUpdate: true,
    updateIntervalSeconds: 3600,
  };

  renderSettings(configWithProviders([mockProvider, remoteProvider]));

  const section = screen.getByTestId("remote-providers-section");
  expect(section).toBeInTheDocument();
  expect(section).toHaveTextContent("Remote Kimi");
});

test("remote_provider_registry_settings_are_loaded_from_config", () => {
  renderSettings({
    ...configWithProviders([]),
    remoteProviderRegistry: {
      registryUrl: "https://example.com/registry.json",
      providerProxyUrl: "http://proxy:8080",
      autoUpdate: false,
    },
  });

  expect(screen.getByTestId("remote-provider-url-input")).toHaveValue(
    "https://example.com/registry.json",
  );
  expect(screen.getByTestId("remote-provider-proxy-url-input")).toHaveValue(
    "http://proxy:8080",
  );
  expect(screen.getByRole("checkbox", { name: "Auto-update when available" })).not.toBeChecked();
});

test("remote_provider_card_uses_visual_window_display_settings", () => {
  const remoteProvider = {
    id: "remote-kimi",
    name: "Remote Kimi",
    enabled: true,
    kind: "remote" as const,
    manifestUrl: "https://example.com/provider.json",
    sourceUrl: "https://example.com/provider.cjs",
    runtime: "node",
    autoUpdate: true,
    updateIntervalSeconds: 3600,
    visibleWindowIds: ["usage"],
    windowLabelOverrides: {},
  };

  renderSettings(configWithProviders([remoteProvider]), [
    providerSnapshot("remote-kimi", [
      quotaWindow("usage", "Usage"),
      quotaWindow("daily", "Daily"),
    ]),
  ]);

  fireEvent.click(screen.getByTestId("edit-provider-remote-kimi"));
  expect(
    screen.getByRole("heading", { name: "Window display" }),
  ).toBeInTheDocument();
  expect(screen.getByLabelText("Show daily")).not.toBeChecked();

  fireEvent.click(screen.getByLabelText("Show daily"));
  fireEvent.change(screen.getByLabelText("Custom label for daily"), {
    target: { value: "Team daily" },
  });

  expect(screen.getByLabelText("Show daily")).toBeChecked();
  expect(screen.getByLabelText("Custom label for daily")).toHaveValue(
    "Team daily",
  );
});

test("remote_provider_name_is_editable", () => {
  const remoteProvider = {
    id: "remote-kimi",
    name: "Remote Kimi",
    enabled: true,
    kind: "remote" as const,
    manifestUrl: "https://example.com/provider.json",
    sourceUrl: "https://example.com/provider.cjs",
    runtime: "node",
    autoUpdate: true,
    updateIntervalSeconds: 3600,
  };

  renderSettings(configWithProviders([remoteProvider]));

  fireEvent.click(screen.getByTestId("edit-provider-remote-kimi"));
  fireEvent.change(screen.getByLabelText("Name"), {
    target: { value: "Team Kimi" },
  });

  expect(screen.getByLabelText("Name")).toHaveValue("Team Kimi");
  expect(screen.getAllByText("Team Kimi").length).toBeGreaterThanOrEqual(1);
});
