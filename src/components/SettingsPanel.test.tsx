import { useState } from "react";
import { fireEvent, render, screen } from "@testing-library/react";
import { afterEach, vi } from "vitest";
import { SettingsPanel } from "./SettingsPanel";
import type { AppConfig, ConfigStorageInfo, ProviderPreset } from "../types";

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
    weekly: "Weekly limit"
  },
  visibleWindowIds: ["5h", "Weekly limit"]
};

const mockProvider = {
  id: "mock-codex",
  name: "Codex Mock",
  enabled: true,
  kind: "mock" as const
};

const presets: ProviderPreset[] = [
  {
    id: "codex-usage",
    displayName: "Codex Usage",
    description: "Codex usage",
    requiredEnvVars: ["CODEX_ACCESS_TOKEN"],
    providerConfigTemplate: codexProvider
  }
];

const configStorageInfo: ConfigStorageInfo = {
  mode: "app-data",
  configPath: "C:\\Users\\tester\\AppData\\Roaming\\QuotaBarWin\\config.quotaBarWin.json",
  configDir: "C:\\Users\\tester\\AppData\\Roaming\\QuotaBarWin",
  appDataConfigPath: "C:\\Users\\tester\\AppData\\Roaming\\QuotaBarWin\\config.quotaBarWin.json",
  portableConfigPath: "C:\\Tools\\QuotaBarWin\\config.quotaBarWin.json",
  portableMarkerPath: "C:\\Tools\\QuotaBarWin\\quotabarwin.portable"
};

function configWithProviders(providers: AppConfig["providers"]): AppConfig {
  return {
    schemaVersion: 8,
    refreshIntervalSeconds: 300,
    displayMode: "remaining",
    lowQuotaWarningThreshold: 20,
    language: "system",
    networkProxy: null,
    providers
  };
}

function renderSettings(initialConfig = configWithProviders([codexProvider])) {
  function Harness() {
    const [config, setConfig] = useState(initialConfig);
    return (
      <SettingsPanel
        config={config}
        configStorageInfo={configStorageInfo}
        isConfigStorageBusy={false}
        isSaving={false}
        onChange={setConfig}
        onOpenConfigFolder={async () => undefined}
        onResetConfig={async () => undefined}
        onSave={() => undefined}
        onSetPortableMode={() => undefined}
        presets={presets}
      />
    );
  }

  return render(<Harness />);
}

test("settings_can_render_codex_provider", () => {
  renderSettings();

  expect(screen.getByLabelText("Refresh interval (seconds)")).toHaveValue(300);
  fireEvent.click(screen.getByRole("button", { name: "Edit" }));
  expect(screen.getByDisplayValue("Codex")).toBeInTheDocument();
  expect(screen.getByLabelText("Auth token")).toHaveValue("${env:CODEX_ACCESS_TOKEN}");
  expect(screen.getByLabelText("ChatGPT account id")).toHaveValue("");
  expect(screen.getByLabelText("Proxy URL")).toHaveValue("");
  expect(screen.getByLabelText("Window label overrides")).toHaveValue(
    "5h=5h\nweekly=Weekly limit"
  );
  expect(screen.getByLabelText("Displayed windows")).toHaveValue("5h\nWeekly limit");
});

test("settings_edits_window_label_overrides", () => {
  renderSettings();

  fireEvent.click(screen.getByRole("button", { name: "Edit" }));
  const overrides = screen.getByLabelText("Window label overrides");
  fireEvent.change(overrides, {
    target: { value: "weekly=Team weekly limit\n5h=5h" }
  });

  expect(screen.getByLabelText("Window label overrides")).toHaveValue(
    "weekly=Team weekly limit\n5h=5h"
  );
});

test("settings_keeps_label_override_newlines_while_editing", () => {
  renderSettings();

  fireEvent.click(screen.getByRole("button", { name: "Edit" }));
  const overrides = screen.getByLabelText("Window label overrides");
  fireEvent.change(overrides, {
    target: { value: "weekly=Team weekly limit\n" }
  });

  expect(screen.getByLabelText("Window label overrides")).toHaveValue(
    "weekly=Team weekly limit\n"
  );
});

test("settings_shows_add_provider_without_local_script_presets", () => {
  renderSettings();

  expect(screen.getByRole("heading", { name: "Add Provider" })).toBeInTheDocument();
  expect(screen.getByRole("button", { name: "Codex Usage" })).toBeInTheDocument();
  expect(screen.queryByRole("button", { name: "Kimi Coding Usage" })).not.toBeInTheDocument();
  expect(screen.queryByRole("button", { name: "OpenCode Quota Command" })).not.toBeInTheDocument();
  expect(screen.queryByRole("button", { name: "Custom Script Provider" })).not.toBeInTheDocument();
});

test("add_codex_preset_shows_token_fields", () => {
  renderSettings(configWithProviders([]));

  fireEvent.click(screen.getByRole("button", { name: "Codex Usage" }));

  expect(screen.getByText("Set CODEX_ACCESS_TOKEN")).toBeInTheDocument();
  expect(screen.getByLabelText("Auth token")).toHaveValue("${env:CODEX_ACCESS_TOKEN}");
  expect(screen.getByLabelText("Window label overrides")).toHaveValue(
    "5h=5h\nweekly=Weekly limit"
  );
});

test("codex_provider_proxy_url_is_editable", () => {
  renderSettings(configWithProviders([]));

  fireEvent.click(screen.getByRole("button", { name: "Codex Usage" }));
  fireEvent.change(screen.getByLabelText("Proxy URL"), {
    target: { value: "socks5h://127.0.0.1:7890" }
  });

  expect(screen.getByLabelText("Proxy URL")).toHaveValue("socks5h://127.0.0.1:7890");
});

test("settings_shows_config_storage_info_and_remote_guide_entry", () => {
  renderSettings();

  expect(screen.getByTestId("general-settings-section")).toBeInTheDocument();
  expect(screen.getByTestId("providers-settings-section")).toBeInTheDocument();
  expect(screen.getByTestId("advanced-settings-section")).not.toHaveAttribute("open");
  expect(screen.getByLabelText("Configuration storage")).toBeInTheDocument();
  expect(screen.getByRole("button", { name: configStorageInfo.configPath })).toBeInTheDocument();
  expect(screen.getByText("AppData mode")).toBeInTheDocument();
  expect(screen.queryByRole("heading", { name: "Custom Provider Guide" })).not.toBeInTheDocument();
  expect(screen.getByRole("heading", { name: "Remote Providers" })).toBeInTheDocument();
  expect(screen.getByRole("button", { name: "Open Guide" })).toBeInTheDocument();
});

test("settings_edits_visible_windows", () => {
  renderSettings({
    ...configWithProviders([codexProvider]),
    providers: [
      {
        ...codexProvider,
        visibleWindowIds: ["weekly"]
      }
    ]
  });

  fireEvent.click(screen.getByRole("button", { name: "Edit" }));
  const visibleWindows = screen.getByLabelText("Displayed windows");
  expect(visibleWindows).toHaveValue("weekly");

  fireEvent.change(visibleWindows, {
    target: { value: "weekly\n5h" }
  });

  expect(screen.getByLabelText("Displayed windows")).toHaveValue("weekly\n5h");
});

test("settings_reorders_providers", () => {
  renderSettings(
    configWithProviders([
      codexProvider,
      {
        ...codexProvider,
        id: "codex-2",
        name: "Second Codex"
      }
    ])
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

  fireEvent.change(screen.getByRole("spinbutton", { name: /Refresh interval/ }), {
    target: { value: "0" }
  });

  expect(screen.getByText("Refresh interval must be greater than 0.")).toBeInTheDocument();
  expect(screen.getByTestId("save-settings-button")).toBeDisabled();

  fireEvent.change(screen.getByRole("spinbutton", { name: /Refresh interval/ }), {
    target: { value: "120" }
  });

  expect(screen.getByTestId("fixed-save-bar")).toHaveTextContent("Unsaved changes");
  expect(screen.getByTestId("save-settings-button")).toBeEnabled();
});

test("network_proxy_section_renders_and_switches_proxy_kind", () => {
  renderSettings(configWithProviders([mockProvider]));

  expect(screen.getByTestId("proxy-kind-select")).toBeInTheDocument();

  fireEvent.change(screen.getByTestId("proxy-kind-select"), { target: { value: "http" } });

  expect(screen.getByTestId("proxy-url-input")).toBeInTheDocument();
});

test("settings_edits_language_choice", () => {
  renderSettings(configWithProviders([mockProvider]));

  fireEvent.change(screen.getByTestId("language-select"), { target: { value: "zh-CN" } });

  expect(screen.getByTestId("language-select")).toHaveValue("zh-CN");
  expect(screen.getByTestId("fixed-save-bar")).toHaveTextContent("Unsaved changes");
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
    updateIntervalSeconds: 3600
  };

  renderSettings(configWithProviders([mockProvider, remoteProvider]));

  const section = screen.getByTestId("remote-providers-section");
  expect(section).toBeInTheDocument();
  expect(section).toHaveTextContent("Remote Kimi");
});
