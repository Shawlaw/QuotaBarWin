import { useState } from "react";
import { fireEvent, render, screen } from "@testing-library/react";
import { SettingsPanel } from "./SettingsPanel";
import type { AppConfig, ProviderPreset, ProviderSnapshot } from "../types";

const commandProvider = {
  id: "command-1",
  name: "Local Command",
  enabled: true,
  kind: "command" as const,
  command: {
    executable: "node",
    args: ["fixtures/fake_provider_snapshot.js"],
    cwd: null,
    env: {},
    timeoutMs: 15000
  },
  parser: { type: "provider-snapshot" as const }
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
      parser: { type: "kimi-coding-usage-v1" }
    }
  },
  {
    id: "bigmodel-zai-coding-plan",
    displayName: "BigModel / Z.ai Coding Plan",
    description: "BigModel usage",
    requiredEnvVars: ["BIGMODEL_API_KEY"],
    providerConfigTemplate: {
      id: "bigmodel-coding-plan",
      name: "BigModel / Z.ai Coding Plan",
      enabled: true,
      kind: "command",
      command: {
        executable: "curl",
        args: ["-H", "Authorization: Bearer ${env:BIGMODEL_API_KEY}"],
        timeoutMs: 15000
      },
      parser: { type: "bigmodel-quota-limit-json-v1" }
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
      parser: { type: "app-snapshot" }
    }
  },
  {
    id: "custom-command-provider",
    displayName: "Custom Command Provider",
    description: "Custom command",
    providerConfigTemplate: commandProvider
  }
];

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
        appVersion="0.0.0"
        isSaving={false}
        onChange={setConfig}
        onClose={() => undefined}
        onSave={() => undefined}
        onTestProvider={testProvider}
        presets={presets}
      />
    );
  }

  return render(<Harness />);
}

test("settings_can_render_command_provider", () => {
  renderSettings();

  expect(screen.getByText("Version 0.0.0")).toBeInTheDocument();
  expect(screen.getByDisplayValue("Local Command")).toBeInTheDocument();
  expect(screen.getByDisplayValue("node")).toBeInTheDocument();
  expect(screen.getByDisplayValue("provider-snapshot")).toBeInTheDocument();
});

test("settings_shows_add_provider", () => {
  renderSettings();

  expect(screen.getByRole("heading", { name: "Add Provider" })).toBeInTheDocument();
  expect(screen.getByRole("button", { name: "Kimi Coding Usage" })).toBeInTheDocument();
  expect(screen.getByRole("button", { name: "OpenCode Quota Command" })).toBeInTheDocument();
});

test("add_kimi_preset_shows_env_hint", () => {
  renderSettings(configWithProviders([]));

  fireEvent.click(screen.getByRole("button", { name: "Kimi Coding Usage" }));

  expect(screen.getByText("Set KIMI_API_KEY")).toBeInTheDocument();
});

test("add_bigmodel_preset_shows_env_hint", () => {
  renderSettings(configWithProviders([]));

  fireEvent.click(screen.getByRole("button", { name: "BigModel / Z.ai Coding Plan" }));

  expect(screen.getByText("Set BIGMODEL_API_KEY")).toBeInTheDocument();
});

test("add_custom_command_provider", () => {
  renderSettings(configWithProviders([]));

  fireEvent.click(screen.getByRole("button", { name: "Custom Command Provider" }));

  expect(screen.getByDisplayValue("Local Command")).toBeInTheDocument();
});
