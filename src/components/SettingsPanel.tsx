import type { AppConfig, CommandProviderConfig, ParserSpec, ProviderConfig } from "../types";

type SettingsPanelProps = {
  config: AppConfig;
  isSaving: boolean;
  onChange: (config: AppConfig) => void;
  onClose: () => void;
  onSave: () => void;
};

function updateProvider(
  config: AppConfig,
  providerId: string,
  updater: (provider: ProviderConfig) => ProviderConfig
): AppConfig {
  return {
    ...config,
    providers: config.providers.map((provider) =>
      provider.id === providerId ? updater(provider) : provider
    )
  };
}

function argsToText(args: string[]): string {
  return args.join(" ");
}

function textToArgs(text: string): string[] {
  return text
    .split(" ")
    .map((part) => part.trim())
    .filter(Boolean);
}

function parserFromType(type: string): ParserSpec {
  if (type === "app-snapshot") {
    return { type };
  }
  if (type === "kimi-coding-usage-v1") {
    return { type };
  }
  if (type === "bigmodel-quota-limit-json-v1") {
    return { type };
  }

  return { type: "provider-snapshot" };
}

export function SettingsPanel({ config, isSaving, onChange, onClose, onSave }: SettingsPanelProps) {
  function addCommandProvider() {
    const id = `command-${Date.now()}`;
    onChange({
      ...config,
      providers: [
        ...config.providers,
        {
          id,
          name: "Command Provider",
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
    });
  }

  function updateCommandProvider(
    provider: CommandProviderConfig,
    patch: Partial<CommandProviderConfig>
  ) {
    onChange(
      updateProvider(config, provider.id, (current) =>
        current.kind === "command" ? { ...current, ...patch } : current
      )
    );
  }

  return (
    <section className="settings-panel" aria-label="Settings">
      <div className="settings-panel__header">
        <h2>Settings</h2>
        <button type="button" className="button-secondary" onClick={onClose}>
          Close
        </button>
      </div>
      <div className="settings-grid">
        <label>
          Refresh interval
          <input
            type="number"
            min={10}
            value={config.refreshIntervalSeconds}
            onChange={(event) =>
              onChange({
                ...config,
                refreshIntervalSeconds: Number(event.currentTarget.value)
              })
            }
          />
        </label>
        <label>
          Display mode
          <select
            value={config.displayMode}
            onChange={(event) =>
              onChange({
                ...config,
                displayMode: event.currentTarget.value as AppConfig["displayMode"]
              })
            }
          >
            <option value="remaining">Remaining</option>
            <option value="used">Used</option>
          </select>
        </label>
        <label>
          Low quota warning
          <input
            type="number"
            min={0}
            max={100}
            value={config.lowQuotaWarningThreshold}
            onChange={(event) =>
              onChange({
                ...config,
                lowQuotaWarningThreshold: Number(event.currentTarget.value)
              })
            }
          />
        </label>
      </div>
      <div className="settings-provider-list">
        {config.providers.map((provider) => (
          <article className="settings-provider" key={provider.id}>
            <label>
              <input
                type="checkbox"
                checked={provider.enabled}
                onChange={(event) =>
                  onChange(
                    updateProvider(config, provider.id, (current) => ({
                      ...current,
                      enabled: event.currentTarget.checked
                    }))
                  )
                }
              />
              {provider.name}
            </label>
            <span>{provider.kind}</span>
            {provider.kind === "command" ? (
              <div className="command-fields">
                <label>
                  Name
                  <input
                    value={provider.name}
                    onChange={(event) =>
                      updateCommandProvider(provider, { name: event.currentTarget.value })
                    }
                  />
                </label>
                <label>
                  Executable
                  <input
                    value={provider.command.executable}
                    onChange={(event) =>
                      updateCommandProvider(provider, {
                        command: {
                          ...provider.command,
                          executable: event.currentTarget.value
                        }
                      })
                    }
                  />
                </label>
                <label>
                  Args
                  <input
                    value={argsToText(provider.command.args)}
                    onChange={(event) =>
                      updateCommandProvider(provider, {
                        command: {
                          ...provider.command,
                          args: textToArgs(event.currentTarget.value)
                        }
                      })
                    }
                  />
                </label>
                <label>
                  Timeout
                  <input
                    type="number"
                    min={100}
                    value={provider.command.timeoutMs}
                    onChange={(event) =>
                      updateCommandProvider(provider, {
                        command: {
                          ...provider.command,
                          timeoutMs: Number(event.currentTarget.value)
                        }
                      })
                    }
                  />
                </label>
                <label>
                  Parser
                  <select
                    value={provider.parser.type}
                    onChange={(event) =>
                      updateCommandProvider(provider, {
                        parser: parserFromType(event.currentTarget.value)
                      })
                    }
                  >
                    <option value="provider-snapshot">provider-snapshot</option>
                    <option value="app-snapshot">app-snapshot</option>
                    <option value="kimi-coding-usage-v1">kimi-coding-usage-v1</option>
                    <option value="bigmodel-quota-limit-json-v1">
                      bigmodel-quota-limit-json-v1
                    </option>
                  </select>
                </label>
              </div>
            ) : null}
          </article>
        ))}
      </div>
      <div className="settings-actions">
        <button type="button" className="button-secondary" onClick={addCommandProvider}>
          Add command provider
        </button>
        <button type="button" onClick={onSave} disabled={isSaving}>
          {isSaving ? "Saving" : "Save"}
        </button>
      </div>
    </section>
  );
}
