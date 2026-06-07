import { render, screen } from "@testing-library/react";
import { SettingsPanel } from "./SettingsPanel";
import type { AppConfig } from "../types";

const config: AppConfig = {
  schemaVersion: 1,
  refreshIntervalSeconds: 300,
  displayMode: "remaining",
  lowQuotaWarningThreshold: 20,
  providers: [
    {
      id: "command-1",
      name: "Local Command",
      enabled: true,
      kind: "command",
      command: {
        executable: "node",
        args: ["fixtures/fake_provider_snapshot.js"],
        cwd: null,
        env: {},
        timeoutMs: 15000
      },
      parser: { type: "provider-snapshot" }
    }
  ]
};

test("settings_can_render_command_provider", () => {
  render(
    <SettingsPanel
      config={config}
      isSaving={false}
      onChange={() => undefined}
      onClose={() => undefined}
      onSave={() => undefined}
    />
  );

  expect(screen.getByDisplayValue("Local Command")).toBeInTheDocument();
  expect(screen.getByDisplayValue("node")).toBeInTheDocument();
  expect(screen.getByDisplayValue("provider-snapshot")).toBeInTheDocument();
});
