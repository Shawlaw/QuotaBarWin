import { useState } from "react";
import { fireEvent, render, screen } from "@testing-library/react";
import { afterEach, vi } from "vitest";
import { SettingsPanel } from "./SettingsPanel";
import type { AppConfig, ConfigStorageInfo, ProviderPreset, ProviderSnapshot } from "../types";

afterEach(() => {
  vi.restoreAllMocks();
});

const commandProvider = {
  id: "command-1",
  name: "Local Command",
  enabled: true,
  kind: "command" as const,
  command: {
    executable: "node",
    args: ["-H", "Authorization: Bearer ${file:C:\\Secrets\\kimi.key}", "fixtures/fake_provider_snapshot.js"],
    cwd: null,
    env: {},
    timeoutMs: 15000
  },
  parser: { type: "provider-snapshot" as const },
  windowLabelOverrides: {},
  visibleWindowIds: []
};

const scriptProvider = {
  id: "script-1",
  name: "Local Script",
  enabled: true,
  kind: "script" as const,
  command: {
    executable: "node",
    args: ["providers/custom/provider.cjs"],
    cwd: null,
    env: {},
    timeoutMs: 15000
  },
  output: { type: "provider-snapshot-v1" as const },
  windowLabelOverrides: {},
  visibleWindowIds: []
};

const presets: ProviderPreset[] = [
  {
    id: "codex-usage",
    displayName: "Codex Usage",
    description: "Codex usage",
    requiredEnvVars: ["CODEX_ACCESS_TOKEN"],
    providerConfigTemplate: {
      id: "codex",
      name: "Codex",
      enabled: true,
      kind: "codex",
      authToken: "${env:CODEX_ACCESS_TOKEN}",
      accountId: null,
      proxyUrl: null,
      timeoutMs: 15000,
      windowLabelOverrides: {
        "5h": "5h",
        weekly: "Weekly limit"
      },
      visibleWindowIds: ["5h", "Weekly limit"]
    }
  },
  {
    id: "kimi-coding-usage",
    displayName: "Kimi Coding Usage",
    description: "Kimi usage",
    requiredEnvVars: ["KIMI_API_KEY"],
    providerConfigTemplate: {
      id: "kimi-coding",
      name: "Kimi Coding",
      enabled: true,
      kind: "script",
      command: {
        executable: "node",
        args: ["C:\\QuotaBarWin\\providers\\builtin\\kimi-coding\\provider.cjs"],
        timeoutMs: 15000
      },
      output: { type: "provider-snapshot-v1" },
      windowLabelOverrides: {},
      visibleWindowIds: []
    }
  },
  {
    id: "bigmodel-coding-plan",
    displayName: "BigModel Coding Plan",
    description: "BigModel usage",
    requiredEnvVars: ["BIGMODEL_API_KEY"],
    providerConfigTemplate: {
      id: "bigmodel-coding-plan",
      name: "BigModel Coding Plan",
      enabled: true,
      kind: "script",
      command: {
        executable: "node",
        args: ["C:\\QuotaBarWin\\providers\\builtin\\bigmodel-coding-plan\\provider.cjs"],
        timeoutMs: 15000
      },
      output: { type: "provider-snapshot-v1" },
      windowLabelOverrides: {
        "tokens-limit-3-5": "5h",
        "tokens-limit-6-1": "Weekly limit",
        "time-limit-5-1": "Monthly time limit"
      },
      visibleWindowIds: []
    }
  },
  {
    id: "opencode-quota-command",
    displayName: "OpenCode Quota Command",
    description: "OpenCode quota",
    providerConfigTemplate: {
      id: "opencode-quota",
      name: "OpenCode Quota",
      enabled: true,
      kind: "script",
      command: {
        executable: "opencode-quota",
        args: ["show", "--json"],
        timeoutMs: 15000
      },
      output: { type: "app-snapshot-v1" },
      windowLabelOverrides: {},
      visibleWindowIds: []
    }
  },
  {
    id: "custom-script-provider",
    displayName: "Custom Script Provider",
    description: "Custom script",
    providerConfigTemplate: scriptProvider
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
    schemaVersion: 1,
    refreshIntervalSeconds: 300,
    displayMode: "remaining",
    lowQuotaWarningThreshold: 20,
    providers
  };
}

function renderSettings(initialConfig = configWithProviders([commandProvider])) {
  const testProvider = async (provider: AppConfig["providers"][number]): Promise<ProviderSnapshot> => ({
    id: provider.id,
    name: provider.name,
    status: "ok",
    source:
      provider.kind === "codex"
        ? "native"
        : provider.kind === "command"
          ? "command"
          : provider.kind === "script"
            ? "script"
            : "mock",
    updatedAt: null,
    windows: [],
    error: null,
    diagnostics: null,
    metadata: null
  });

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
        onOpenCustomProviderGuide={async () => undefined}
        onResetConfig={async () => undefined}
        onSave={() => undefined}
        onSetPortableMode={() => undefined}
        onTestProvider={testProvider}
        presets={presets}
      />
    );
  }

  return render(<Harness />);
}

test("settings_can_render_command_provider", () => {
  renderSettings();

  expect(screen.getByLabelText("Refresh interval (seconds)")).toHaveValue(300);
  fireEvent.click(screen.getByRole("button", { name: "Edit" }));
  expect(screen.getByDisplayValue("Local Command")).toBeInTheDocument();
  expect(screen.getByDisplayValue("node")).toBeInTheDocument();
  expect(screen.getByLabelText("Args")).toHaveValue(
    "-H\nAuthorization: Bearer ${file:C:\\Secrets\\kimi.key}\nfixtures/fake_provider_snapshot.js"
  );
  expect(screen.getByDisplayValue("provider-snapshot")).toBeInTheDocument();
});

test("settings_can_render_script_provider_contract", () => {
  renderSettings(configWithProviders([scriptProvider]));

  fireEvent.click(screen.getByRole("button", { name: "Edit" }));

  expect(screen.getByDisplayValue("Local Script")).toBeInTheDocument();
  expect(screen.getByTestId("script-executable-script-1")).toHaveValue("node");
  expect(screen.getByLabelText("Args")).toHaveValue("providers/custom/provider.cjs");
  expect(screen.getByLabelText("Output contract")).toHaveValue("provider-snapshot-v1");

  fireEvent.change(screen.getByLabelText("Output contract"), {
    target: { value: "app-snapshot-v1" }
  });

  expect(screen.getByLabelText("Output contract")).toHaveValue("app-snapshot-v1");
  expect(screen.queryByLabelText("Parser")).not.toBeInTheDocument();
});

test("settings_edits_window_label_overrides", () => {
  renderSettings({
    ...configWithProviders([commandProvider]),
    providers: [
      {
        ...commandProvider,
        windowLabelOverrides: {
          weekly: "Weekly limit"
        }
      }
    ]
  });

  fireEvent.click(screen.getByRole("button", { name: "Edit" }));
  const overrides = screen.getByLabelText("Window label overrides");
  expect(overrides).toHaveValue("weekly=Weekly limit");

  fireEvent.change(overrides, {
    target: { value: "weekly=Team weekly limit\n300-minute=5h" }
  });

  expect(screen.getByLabelText("Window label overrides")).toHaveValue(
    "weekly=Team weekly limit\n300-minute=5h"
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

test("settings_shows_mapping_and_displayed_window_hints", () => {
  renderSettings();

  fireEvent.click(screen.getByRole("button", { name: "Edit" }));
  expect(screen.getByLabelText("Window label overrides")).toHaveAttribute(
    "placeholder",
    "window-id=Display name\n300-minute=5h\ntokens-limit-6-1=Weekly limit"
  );
  expect(screen.getByLabelText("Displayed windows")).toHaveAttribute(
    "placeholder",
    "Leave empty to show all\n5h\ntokens-limit-3-5\nWeekly limit"
  );
});

test("settings_shows_add_provider", () => {
  renderSettings();

  expect(screen.getByRole("heading", { name: "Add Provider" })).toBeInTheDocument();
  expect(screen.getByRole("button", { name: "Codex Usage" })).toBeInTheDocument();
  expect(screen.getByRole("button", { name: "Kimi Coding Usage" })).toBeInTheDocument();
  expect(screen.getByRole("button", { name: "OpenCode Quota Command" })).toBeInTheDocument();
});

test("add_codex_preset_shows_token_fields", () => {
  renderSettings(configWithProviders([]));

  fireEvent.click(screen.getByRole("button", { name: "Codex Usage" }));

  expect(screen.getByText("Set CODEX_ACCESS_TOKEN")).toBeInTheDocument();
  expect(screen.getByLabelText("Auth token")).toHaveValue("${env:CODEX_ACCESS_TOKEN}");
  expect(screen.getByLabelText("ChatGPT account id")).toHaveValue("");
  expect(screen.getByLabelText("Proxy URL")).toHaveValue("");
  expect(screen.getByLabelText("Window label overrides")).toHaveValue(
    "5h=5h\nweekly=Weekly limit"
  );
  expect(screen.getByLabelText("Displayed windows")).toHaveValue("5h\nWeekly limit");
});

test("codex_provider_proxy_url_is_editable", () => {
  renderSettings(configWithProviders([]));

  fireEvent.click(screen.getByRole("button", { name: "Codex Usage" }));
  fireEvent.change(screen.getByLabelText("Proxy URL"), {
    target: { value: "socks5h://127.0.0.1:7890" }
  });

  expect(screen.getByLabelText("Proxy URL")).toHaveValue("socks5h://127.0.0.1:7890");
});

test("settings_shows_config_storage_info_and_provider_guide_entry", () => {
  renderSettings();

  expect(screen.getByTestId("general-settings-section")).toBeInTheDocument();
  expect(screen.getByTestId("providers-settings-section")).toBeInTheDocument();
  expect(screen.getByTestId("advanced-settings-section")).not.toHaveAttribute("open");
  expect(screen.getByLabelText("Configuration storage")).toBeInTheDocument();
  expect(screen.getByRole("button", { name: configStorageInfo.configPath })).toBeInTheDocument();
  expect(screen.getByText("AppData mode")).toBeInTheDocument();
  expect(screen.getByRole("heading", { name: "Custom Provider Guide" })).toBeInTheDocument();
  expect(screen.getByRole("button", { name: "Open Guide" })).toBeInTheDocument();
});

test("add_kimi_preset_shows_env_hint", () => {
  renderSettings(configWithProviders([]));

  fireEvent.click(screen.getByRole("button", { name: "Kimi Coding Usage" }));

  expect(screen.getByText("Set KIMI_API_KEY")).toBeInTheDocument();
});

test("add_bigmodel_preset_shows_env_hint", () => {
  renderSettings(configWithProviders([]));

  fireEvent.click(screen.getByRole("button", { name: "BigModel Coding Plan" }));

  expect(screen.getByText("Set BIGMODEL_API_KEY")).toBeInTheDocument();
  expect(screen.getByLabelText("Window label overrides")).toHaveValue(
    "tokens-limit-3-5=5h\ntokens-limit-6-1=Weekly limit\ntime-limit-5-1=Monthly time limit"
  );
});

test("settings_edits_visible_windows", () => {
  renderSettings({
    ...configWithProviders([commandProvider]),
    providers: [
      {
        ...commandProvider,
        visibleWindowIds: ["weekly"]
      }
    ]
  });

  fireEvent.click(screen.getByRole("button", { name: "Edit" }));
  const visibleWindows = screen.getByLabelText("Displayed windows");
  expect(visibleWindows).toHaveValue("weekly");

  fireEvent.change(visibleWindows, {
    target: { value: "weekly\n300-minute" }
  });

  expect(screen.getByLabelText("Displayed windows")).toHaveValue("weekly\n300-minute");
});

test("settings_keeps_displayed_window_newlines_while_editing", () => {
  renderSettings();

  fireEvent.click(screen.getByRole("button", { name: "Edit" }));
  const visibleWindows = screen.getByLabelText("Displayed windows");
  fireEvent.change(visibleWindows, {
    target: { value: "5h\n" }
  });

  expect(screen.getByLabelText("Displayed windows")).toHaveValue("5h\n");
});

test("settings_reorders_providers", () => {
  renderSettings(
    configWithProviders([
      commandProvider,
      {
        ...commandProvider,
        id: "command-2",
        name: "Second Command"
      }
    ])
  );

  fireEvent.click(screen.getAllByRole("button", { name: "More" })[1]);
  fireEvent.click(screen.getByRole("button", { name: "Up" }));

  expect(screen.getByText("Second Command")).toBeInTheDocument();
});

test("add_custom_script_provider", () => {
  renderSettings(configWithProviders([]));

  fireEvent.click(screen.getByRole("button", { name: "Custom Script Provider" }));

  expect(screen.getByDisplayValue("Local Script")).toBeInTheDocument();
  expect(screen.getByLabelText("Output contract")).toHaveValue("provider-snapshot-v1");
});

test("remove_provider_deletes_provider_from_settings", () => {
  vi.spyOn(window, "confirm").mockReturnValue(true);
  renderSettings();

  fireEvent.click(screen.getByRole("button", { name: "More" }));
  fireEvent.click(screen.getByRole("button", { name: "Remove" }));

  expect(screen.queryByText("Local Command")).not.toBeInTheDocument();
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
