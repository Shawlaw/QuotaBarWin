import { useState } from "react";
import type {
  AppConfig,
  CommandProviderConfig,
  ParserSpec,
  ProviderConfig,
  ProviderPreset,
  ProviderSnapshot
} from "../types";

type SettingsPanelProps = {
  config: AppConfig;
  isSaving: boolean;
  presets: ProviderPreset[];
  onChange: (config: AppConfig) => void;
  onClose: () => void;
  onSave: () => void;
  onTestProvider: (provider: ProviderConfig) => Promise<ProviderSnapshot>;
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

function cloneProvider(provider: ProviderConfig): ProviderConfig {
  return JSON.parse(JSON.stringify(provider)) as ProviderConfig;
}

export function SettingsPanel({
  config,
  isSaving,
  presets,
  onChange,
  onClose,
  onSave,
  onTestProvider
}: SettingsPanelProps) {
  const [testResults, setTestResults] = useState<Record<string, ProviderSnapshot>>({});

  function addPreset(preset: ProviderPreset) {
    onChange({
      ...config,
      providers: [...config.providers, cloneProvider(preset.providerConfigTemplate)]
    });
  }

  function addCommandProvider() {
    const custom = presets.find((preset) => preset.id === "custom-command-provider");
    if (custom) {
      addPreset(custom);
    }
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
      <section className="preset-list" aria-label="Add Provider">
        <h3>Add Provider</h3>
        <div className="preset-actions">
          {presets.map((preset) => (
            <button
              type="button"
              className="button-secondary"
              key={preset.id}
              onClick={() => addPreset(preset)}
            >
              {preset.displayName}
            </button>
          ))}
        </div>
      </section>
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
            {presets
              .find((preset) => preset.providerConfigTemplate.id === provider.id)
              ?.requiredEnvVars?.map((envVar) => (
                <p className="env-hint" key={envVar}>
                  Set {envVar}
                </p>
              ))}
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
            <div className="provider-test">
              <button
                type="button"
                className="button-secondary"
                onClick={async () => {
                  const result = await onTestProvider(provider);
                  setTestResults((current) => ({ ...current, [provider.id]: result }));
                }}
              >
                Test Provider
              </button>
              {testResults[provider.id] ? (
                <pre>{JSON.stringify(testResults[provider.id], null, 2)}</pre>
              ) : null}
            </div>
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
