import { useRef, useState } from "react";
import type {
  AppConfig,
  CodexProviderConfig,
  CommandProviderConfig,
  ConfigStorageInfo,
  ParserSpec,
  ProviderConfig,
  ProviderPreset,
  ProviderSnapshot
} from "../types";

type SettingsPanelProps = {
  config: AppConfig;
  configStorageInfo: ConfigStorageInfo | null;
  isConfigStorageBusy: boolean;
  isSaving: boolean;
  presets: ProviderPreset[];
  onChange: (config: AppConfig) => void;
  onOpenConfigFolder: () => Promise<void>;
  onResetConfig: () => Promise<void>;
  onSave: () => void | Promise<void>;
  onSetPortableMode: (enabled: boolean) => void;
  onTestProvider: (provider: ProviderConfig) => Promise<ProviderSnapshot>;
};

type ProviderTextDraft = {
  windowLabelOverrides?: string;
  visibleWindowIds?: string;
};

type WindowConfigProvider = CommandProviderConfig | CodexProviderConfig;

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

function removeProvider(config: AppConfig, providerId: string): AppConfig {
  return {
    ...config,
    providers: config.providers.filter((provider) => provider.id !== providerId)
  };
}

function moveProvider(config: AppConfig, providerId: string, direction: -1 | 1): AppConfig {
  const fromIndex = config.providers.findIndex((provider) => provider.id === providerId);
  const toIndex = fromIndex + direction;

  if (fromIndex === -1 || toIndex < 0 || toIndex >= config.providers.length) {
    return config;
  }

  const providers = [...config.providers];
  const [provider] = providers.splice(fromIndex, 1);
  providers.splice(toIndex, 0, provider);

  return {
    ...config,
    providers
  };
}

function uniqueProviderId(config: AppConfig, providerId: string): string {
  const existingIds = new Set(config.providers.map((provider) => provider.id));
  if (!existingIds.has(providerId)) {
    return providerId;
  }

  let index = 2;
  while (existingIds.has(`${providerId}-${index}`)) {
    index += 1;
  }

  return `${providerId}-${index}`;
}

function argsToText(args: string[]): string {
  return args.join("\n");
}

function textToArgs(text: string): string[] {
  return text
    .split(/\r?\n/)
    .map((part) => part.trim())
    .filter(Boolean);
}

function visibleWindowsToText(windowIds: string[] | undefined): string {
  return (windowIds ?? []).join("\n");
}

function textToVisibleWindows(text: string): string[] {
  return text
    .split(/\r?\n/)
    .map((line) => line.trim())
    .filter(Boolean);
}

function labelOverridesToText(overrides: Record<string, string> | undefined): string {
  return Object.entries(overrides ?? {})
    .map(([id, label]) => `${id}=${label}`)
    .join("\n");
}

function textToLabelOverrides(text: string): Record<string, string> {
  return Object.fromEntries(
    text
      .split(/\r?\n/)
      .map((line) => line.trim())
      .filter(Boolean)
      .map((line) => {
        const separator = line.indexOf("=");
        if (separator === -1) {
          return null;
        }
        return [line.slice(0, separator).trim(), line.slice(separator + 1).trim()] as const;
      })
      .filter((entry): entry is readonly [string, string] =>
        Boolean(entry && entry[0].length > 0 && entry[1].length > 0)
      )
  );
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
  configStorageInfo,
  isConfigStorageBusy,
  isSaving,
  presets,
  onChange,
  onOpenConfigFolder,
  onResetConfig,
  onSave,
  onSetPortableMode,
  onTestProvider
}: SettingsPanelProps) {
  const [testResults, setTestResults] = useState<Record<string, ProviderSnapshot>>({});
  const [textDrafts, setTextDrafts] = useState<Record<string, ProviderTextDraft>>({});
  const [expandedProviders, setExpandedProviders] = useState<Record<string, boolean>>({});
  const [expandedProviderActions, setExpandedProviderActions] = useState<Record<string, boolean>>({});
  const [saveMessage, setSaveMessage] = useState("No changes");
  const initialConfigRef = useRef(JSON.stringify(config));
  const configDraft = JSON.stringify(config);
  const hasChanges = configDraft !== initialConfigRef.current;
  const refreshIntervalError =
    config.refreshIntervalSeconds > 0 ? null : "Refresh interval must be greater than 0.";
  const lowQuotaWarningError =
    config.lowQuotaWarningThreshold >= 0 && config.lowQuotaWarningThreshold <= 100
      ? null
      : "Low quota warning must be between 0 and 100.";
  const canSave = hasChanges && !refreshIntervalError && !lowQuotaWarningError && !isSaving;

  function setProviderTextDraft(
    providerId: string,
    field: keyof ProviderTextDraft,
    value: string
  ) {
    setTextDrafts((current) => ({
      ...current,
      [providerId]: {
        ...current[providerId],
        [field]: value
      }
    }));
  }

  function windowLabelOverridesText(provider: WindowConfigProvider): string {
    return (
      textDrafts[provider.id]?.windowLabelOverrides ??
      labelOverridesToText(provider.windowLabelOverrides)
    );
  }

  function visibleWindowIdsText(provider: WindowConfigProvider): string {
    return textDrafts[provider.id]?.visibleWindowIds ?? visibleWindowsToText(provider.visibleWindowIds);
  }

  function addPreset(preset: ProviderPreset) {
    const provider = cloneProvider(preset.providerConfigTemplate);
    provider.id = uniqueProviderId(config, provider.id);

    setExpandedProviders((current) => ({ ...current, [provider.id]: true }));
    onChange({
      ...config,
      providers: [...config.providers, provider]
    });
  }

  async function saveSettings() {
    setSaveMessage("Saving");
    try {
      await onSave();
      initialConfigRef.current = JSON.stringify(config);
      setSaveMessage("Saved");
      window.setTimeout(() => setSaveMessage("No changes"), 1600);
    } catch (error) {
      setSaveMessage(error instanceof Error ? error.message : "Save failed");
    }
  }

  function resetChanges() {
    onChange(JSON.parse(initialConfigRef.current) as AppConfig);
    setSaveMessage("No changes");
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

  function updateCodexProvider(
    provider: CodexProviderConfig,
    patch: Partial<CodexProviderConfig>
  ) {
    onChange(
      updateProvider(config, provider.id, (current) =>
        current.kind === "codex" ? { ...current, ...patch } : current
      )
    );
  }

  return (
    <section className="settings-panel" aria-label="Settings" data-testid="settings-page">
      <section className="settings-section" aria-label="General" data-testid="general-settings-section">
        <div className="settings-section-title">
          <h3>General</h3>
        </div>
        <div className="settings-grid">
        <label>
          Refresh interval (seconds)
          <input
            type="number"
            min={1}
            data-testid="refresh-interval-input"
            value={config.refreshIntervalSeconds}
            onChange={(event) =>
              onChange({
                ...config,
                refreshIntervalSeconds: Number(event.currentTarget.value)
              })
            }
          />
          {refreshIntervalError ? <span className="field-error">{refreshIntervalError}</span> : null}
        </label>
        <label>
          Display mode
          <select
            data-testid="display-mode-select"
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
            data-testid="low-quota-warning-input"
            value={config.lowQuotaWarningThreshold}
            onChange={(event) =>
              onChange({
                ...config,
                lowQuotaWarningThreshold: Number(event.currentTarget.value)
              })
            }
          />
          {lowQuotaWarningError ? <span className="field-error">{lowQuotaWarningError}</span> : null}
        </label>
        <label>
          Log level
          <select
            value={config.logLevel ?? "info"}
            onChange={(event) =>
              onChange({
                ...config,
                logLevel: event.currentTarget.value
              })
            }
          >
            <option value="debug">Debug</option>
            <option value="info">Info</option>
            <option value="warn">Warn</option>
            <option value="error">Error</option>
          </select>
        </label>
        <label className="checkbox-row">
          <input
            type="checkbox"
            checked={config.launchAtStartup ?? false}
            onChange={(event) =>
              onChange({
                ...config,
                launchAtStartup: event.currentTarget.checked
              })
            }
          />
          Launch at startup
        </label>
        </div>
      </section>

      <section className="settings-section" aria-label="Providers" data-testid="providers-settings-section">
        <div className="settings-section-title">
          <h3>Providers</h3>
          <span>{config.providers.length} configured</span>
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
        {config.providers.length === 0 ? (
          <p className="settings-empty">No providers yet. Add one to start monitoring quota.</p>
        ) : null}
        {config.providers.map((provider, providerIndex) => (
          <article className="settings-provider" key={provider.id} data-testid={`settings-provider-${provider.id}`}>
            <div className="settings-provider__header">
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
              <button
                type="button"
                className="button-secondary"
                data-testid={`edit-provider-${provider.id}`}
                onClick={() =>
                  setExpandedProviders((current) => ({
                    ...current,
                    [provider.id]: !(current[provider.id] ?? false)
                  }))
                }
              >
                {expandedProviders[provider.id] ? "Collapse" : "Edit"}
              </button>
              <button
                type="button"
                className="button-ghost"
                data-testid={`more-provider-${provider.id}`}
                onClick={() =>
                  setExpandedProviderActions((current) => ({
                    ...current,
                    [provider.id]: !(current[provider.id] ?? false)
                  }))
                }
              >
                More
              </button>
            </div>
            {expandedProviderActions[provider.id] ? (
              <div className="settings-provider__actions">
                <button
                  type="button"
                  className="button-secondary button-compact"
                  disabled={providerIndex === 0}
                  onClick={() => onChange(moveProvider(config, provider.id, -1))}
                >
                  Up
                </button>
                <button
                  type="button"
                  className="button-secondary button-compact"
                  disabled={providerIndex === config.providers.length - 1}
                  onClick={() => onChange(moveProvider(config, provider.id, 1))}
                >
                  Down
                </button>
                <button
                  type="button"
                  className="button-danger button-compact"
                  data-testid={`remove-provider-${provider.id}`}
                  onClick={() => {
                    if (window.confirm(`Remove provider ${provider.name}?`)) {
                      onChange(removeProvider(config, provider.id));
                    }
                  }}
                >
                  Remove
                </button>
              </div>
            ) : null}
            {expandedProviders[provider.id] ? (
              <>
                {presets
                  .find((preset) => preset.providerConfigTemplate.id === provider.id)
                  ?.requiredEnvVars?.map((envVar) => (
                    <p className="env-hint" key={envVar}>
                      Set {envVar}
                    </p>
                  ))}
                {provider.kind === "codex" ? (
                  <div className="command-fields">
                    <label>
                      Name
                      <input
                        value={provider.name}
                        onChange={(event) =>
                          updateCodexProvider(provider, { name: event.currentTarget.value })
                        }
                      />
                    </label>
                    <label className="args-field">
                      Auth token
                      <textarea
                        rows={3}
                        placeholder={"Paste a token, ${env:CODEX_ACCESS_TOKEN}, or ${file:C:\\Secrets\\codex-token.txt}"}
                        value={provider.authToken}
                        onChange={(event) =>
                          updateCodexProvider(provider, { authToken: event.currentTarget.value })
                        }
                      />
                    </label>
                    <label>
                      ChatGPT account id
                      <input
                        placeholder="Optional"
                        value={provider.accountId ?? ""}
                        onChange={(event) =>
                          updateCodexProvider(provider, {
                            accountId: event.currentTarget.value || null
                          })
                        }
                      />
                    </label>
                    <label>
                      Proxy URL
                      <input
                        placeholder="Optional, e.g. http://127.0.0.1:7890 or socks5h://127.0.0.1:7890"
                        value={provider.proxyUrl ?? ""}
                        onChange={(event) =>
                          updateCodexProvider(provider, {
                            proxyUrl: event.currentTarget.value || null
                          })
                        }
                      />
                    </label>
                    <label>
                      Timeout
                      <input
                        type="number"
                        min={100}
                        value={provider.timeoutMs}
                        onChange={(event) =>
                          updateCodexProvider(provider, {
                            timeoutMs: Number(event.currentTarget.value)
                          })
                        }
                      />
                    </label>
                    <label className="args-field">
                      Window label overrides
                      <textarea
                        rows={4}
                        placeholder={"window-id=Display name\n5h=5h\nweekly=Weekly limit"}
                        value={windowLabelOverridesText(provider)}
                        onChange={(event) => {
                          setProviderTextDraft(
                            provider.id,
                            "windowLabelOverrides",
                            event.currentTarget.value
                          );
                          updateCodexProvider(provider, {
                            windowLabelOverrides: textToLabelOverrides(event.currentTarget.value)
                          });
                        }}
                      />
                    </label>
                    <label className="args-field">
                      Displayed windows
                      <textarea
                        rows={3}
                        placeholder={"Leave empty to show all\n5h\nweekly\nWeekly limit"}
                        value={visibleWindowIdsText(provider)}
                        onChange={(event) => {
                          setProviderTextDraft(provider.id, "visibleWindowIds", event.currentTarget.value);
                          updateCodexProvider(provider, {
                            visibleWindowIds: textToVisibleWindows(event.currentTarget.value)
                          });
                        }}
                      />
                    </label>
                  </div>
                ) : null}
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
                <label className="args-field">
                  Args
                  <textarea
                    rows={5}
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
                <label className="args-field">
                  Window label overrides
                  <textarea
                    rows={4}
                    placeholder={"window-id=Display name\n300-minute=5h\ntokens-limit-6-1=Weekly limit"}
                    value={windowLabelOverridesText(provider)}
                    onChange={(event) => {
                      setProviderTextDraft(
                        provider.id,
                        "windowLabelOverrides",
                        event.currentTarget.value
                      );
                      updateCommandProvider(provider, {
                        windowLabelOverrides: textToLabelOverrides(event.currentTarget.value)
                      });
                    }}
                  />
                </label>
                <label className="args-field">
                  Displayed windows
                  <textarea
                    rows={3}
                    placeholder={"Leave empty to show all\n5h\ntokens-limit-3-5\nWeekly limit"}
                    value={visibleWindowIdsText(provider)}
                    onChange={(event) => {
                      setProviderTextDraft(provider.id, "visibleWindowIds", event.currentTarget.value);
                      updateCommandProvider(provider, {
                        visibleWindowIds: textToVisibleWindows(event.currentTarget.value)
                      });
                    }}
                  />
                </label>
                  </div>
                ) : null}
                <div className="provider-test">
                  <button
                    type="button"
                    className="button-secondary"
                    data-testid={`test-provider-${provider.id}`}
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
              </>
            ) : null}
          </article>
        ))}
        </div>
      </section>

      <details className="settings-section settings-advanced" data-testid="advanced-settings-section">
        <summary>Advanced</summary>
        <details className="settings-info" aria-label="Configuration storage">
          <summary>
            Configuration storage
            <span>{configStorageInfo?.mode === "portable" ? "Portable mode" : "AppData mode"}</span>
          </summary>
          <div className="config-paths">
            <span>Config file</span>
            <button
              type="button"
              className="path-chip"
              title={configStorageInfo?.configPath}
              onClick={() => void navigator.clipboard?.writeText(configStorageInfo?.configPath ?? "")}
            >
              {configStorageInfo?.configPath ?? "Loading config path..."}
            </button>
            <span>AppData</span>
            <code title={configStorageInfo?.appDataConfigPath}>{configStorageInfo?.appDataConfigPath ?? "Loading..."}</code>
            <span>Portable</span>
            <code title={configStorageInfo?.portableConfigPath}>{configStorageInfo?.portableConfigPath ?? "Loading..."}</code>
          </div>
          <label className="checkbox-row">
            <input
              type="checkbox"
              checked={configStorageInfo?.mode === "portable"}
              disabled={!configStorageInfo || isConfigStorageBusy}
              onChange={(event) => onSetPortableMode(event.currentTarget.checked)}
            />
            Portable mode
          </label>
          <div className="settings-hint">
            Portable mode stores config beside the app executable and uses quotabarwin.portable as
            the marker file.
          </div>
          <div className="settings-actions settings-actions--inline">
            <button
              type="button"
              className="button-secondary"
              disabled={!configStorageInfo || isConfigStorageBusy}
              onClick={() => void onOpenConfigFolder()}
            >
              Open folder
            </button>
            <button
              type="button"
              className="button-danger"
              disabled={isConfigStorageBusy}
              onClick={() => {
                if (window.confirm("Reset QuotaBarWin config to defaults? A backup will be created first.")) {
                  void onResetConfig();
                }
              }}
            >
              Reset config
            </button>
          </div>
        </details>
        <details className="settings-guide">
          <summary>Custom Provider guide</summary>
          <div className="guide-grid">
            <section>
              <h3>Command</h3>
              <p>
                QuotaBarWin runs Executable with each Args line as one command-line argument.
                stdout is parsed as quota JSON; non-zero exit codes and timeouts become status error.
              </p>
              <p>
                Args and env values can read secrets with {"${env:NAME}"} or {"${file:C:\\Path With Spaces\\secret.txt}"}.
                Quoted file paths also work, for example {"${file:\"C:\\Path With Spaces\\secret.txt\"}"}.
                Secret values are redacted from diagnostics.
              </p>
            </section>
            <section>
              <h3>Parser</h3>
              <dl>
                <dt>provider-snapshot</dt>
                <dd>stdout is one ProviderSnapshot JSON object.</dd>
                <dt>app-snapshot</dt>
                <dd>stdout is an AppSnapshot JSON object with providers.</dd>
                <dt>kimi-coding-usage-v1</dt>
                <dd>stdout is Kimi usage API JSON.</dd>
                <dt>bigmodel-quota-limit-json-v1</dt>
                <dd>stdout is BigModel quota limit API JSON.</dd>
              </dl>
            </section>
          </div>
        </details>
      </details>

      <div className="fixed-save-bar" data-testid="fixed-save-bar">
        <span>{hasChanges ? "Unsaved changes" : saveMessage}</span>
        <div className="settings-actions">
          <button type="button" className="button-secondary" onClick={resetChanges} disabled={!hasChanges || isSaving}>
            Reset changes
          </button>
          <button type="button" onClick={() => void saveSettings()} disabled={!canSave} data-testid="save-settings-button">
            {isSaving ? "Saving" : "Save"}
          </button>
        </div>
      </div>
    </section>
  );
}
