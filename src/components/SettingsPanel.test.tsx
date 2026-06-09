import { useState } from "react";
import { fireEvent, render, screen } from "@testing-library/react";
import { SettingsPanel } from "./SettingsPanel";
import type { AppConfig, ConfigStorageInfo, ProviderPreset, ProviderSnapshot } from "../types";

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

const presets: ProviderPreset[] = [
  {
    id: "kimi-coding-usage",
    displayName: "Kimi Coding Usage",
    description: "Kimi usage",
    requiredEnvVars: ["KIMI_API_KEY"],
    providerConfigTemplate: {
      id: "kimi-coding",
      name: "Kimi Coding",
      enabled: true,
      kind: "command",
      command: {
        executable: "curl",
        args: ["-H", "Authorization: Bearer ${env:KIMI_API_KEY}"],
        timeoutMs: 15000
      },
      parser: { type: "kimi-coding-usage-v1" },
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
      kind: "command",
      command: {
        executable: "curl",
        args: ["-H", "Authorization: Bearer ${env:BIGMODEL_API_KEY}"],
        timeoutMs: 15000
      },
      parser: { type: "bigmodel-quota-limit-json-v1" },
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
      kind: "command",
      command: {
        executable: "opencode-quota",
        args: ["show", "--json"],
        timeoutMs: 15000
      },
      parser: { type: "app-snapshot" },
      windowLabelOverrides: {},
      visibleWindowIds: []
    }
  },
  {
    id: "custom-command-provider",
    displayName: "Custom Command Provider",
    description: "Custom command",
    providerConfigTemplate: commandProvider
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
    source: provider.kind === "command" ? "command" : "mock",
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
  expect(screen.getByRole("button", { name: "Kimi Coding Usage" })).toBeInTheDocument();
  expect(screen.getByRole("button", { name: "OpenCode Quota Command" })).toBeInTheDocument();
});

test("settings_shows_config_storage_info_and_provider_guide", () => {
  renderSettings();

  expect(screen.getByLabelText("Configuration storage")).toBeInTheDocument();
  expect(screen.getByDisplayValue(configStorageInfo.configPath)).toBeInTheDocument();
  expect(screen.getByText("AppData mode")).toBeInTheDocument();
  expect(screen.getByText("Custom Provider guide")).toBeInTheDocument();
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

  fireEvent.click(screen.getAllByRole("button", { name: "Up" })[1]);

  expect(screen.getByText("Second Command")).toBeInTheDocument();
});

test("add_custom_command_provider", () => {
  renderSettings(configWithProviders([]));

  fireEvent.click(screen.getByRole("button", { name: "Custom Command Provider" }));

  expect(screen.getByDisplayValue("Local Command")).toBeInTheDocument();
});

test("remove_provider_deletes_provider_from_settings", () => {
  renderSettings();

  fireEvent.click(screen.getByRole("button", { name: "Remove" }));

  expect(screen.queryByText("Local Command")).not.toBeInTheDocument();
});
