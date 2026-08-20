import { useEffect, useMemo, useState } from "react";
import type {
  LocalApiNetworkAddress,
  LocalApiNetworkInterface,
  LocalApiSettings as LocalApiSettingsValue,
  LocalApiStatus,
} from "../types";
import {
  getLocalApiAccessToken,
  getLocalApiStatus,
  listLocalApiNetworkInterfaces,
  setLocalApiAccessToken,
} from "../lib/api";
import { useI18n } from "../i18n";

const DEFAULT_LOCAL_API_SETTINGS: LocalApiSettingsValue = {
  enabled: false,
  bindTarget: { kind: "loopback" },
  port: 41833,
};

function formatNetworkAddress(address: LocalApiNetworkAddress): string {
  return address.scopeId ? `${address.address}%${address.scopeId}` : address.address;
}

function endpointForNetworkAddress(address: LocalApiNetworkAddress, port: number): string {
  if (!address.address.includes(":")) {
    return `http://${address.address}:${port}`;
  }
  const scope = address.scopeId ? `%25${address.scopeId}` : "";
  return `http://[${address.address}${scope}]:${port}`;
}

type LocalApiSettingsProps = {
  settings: LocalApiSettingsValue | null | undefined;
  refreshKey?: string | number;
  onChange: (settings: LocalApiSettingsValue) => void;
  onTokenRequirementChange?: (tokenRequired: boolean) => void;
};

function normalizedSettings(
  settings: LocalApiSettingsValue | null | undefined,
): LocalApiSettingsValue {
  return settings ?? DEFAULT_LOCAL_API_SETTINGS;
}

export function LocalApiSettings({
  settings: savedSettings,
  refreshKey,
  onChange,
  onTokenRequirementChange,
}: LocalApiSettingsProps) {
  const { t } = useI18n();
  const settings = normalizedSettings(savedSettings);
  const [interfaces, setInterfaces] = useState<LocalApiNetworkInterface[]>([]);
  const [status, setStatus] = useState<LocalApiStatus | null>(null);
  const [interfacesUnavailable, setInterfacesUnavailable] = useState(false);
  const [token, setToken] = useState("");
  const [isReplacingToken, setIsReplacingToken] = useState(false);
  const [tokenMessage, setTokenMessage] = useState<string | null>(null);
  const [isSavingToken, setIsSavingToken] = useState(false);

  const selectedInterfaceIds = useMemo(() => {
    switch (settings.bindTarget.kind) {
      case "network-interface":
        return [settings.bindTarget.adapterId];
      case "network-interfaces":
        return [...new Set(settings.bindTarget.adapterIds)];
      default:
        return [];
    }
  }, [settings.bindTarget]);
  const isSelectedInterfacesTarget =
    settings.bindTarget.kind === "network-interface" ||
    settings.bindTarget.kind === "network-interfaces";
  const isNetworkTarget = settings.bindTarget.kind !== "loopback";
  const includesLoopback =
    settings.bindTarget.kind === "loopback" || settings.bindTarget.includeLoopback !== false;
  const hasNetworkListener =
    settings.bindTarget.kind === "network-interface" ||
    settings.bindTarget.kind === "all-network-interfaces" ||
    (settings.bindTarget.kind === "network-interfaces" && selectedInterfaceIds.length > 0);
  const tokenRequiredBeforeSaving =
    settings.enabled && hasNetworkListener && status?.tokenConfigured !== true;
  const hasSavedToken = status?.tokenConfigured === true;
  const tokenEditorVisible = !hasSavedToken || isReplacingToken;
  const portIsValid =
    Number.isInteger(settings.port) && settings.port >= 1 && settings.port <= 65535;

  const predictedEndpoints = useMemo(() => {
    if (settings.bindTarget.kind === "loopback") {
      return [`http://127.0.0.1:${settings.port}`];
    }
    const selectedIds =
      settings.bindTarget.kind === "all-network-interfaces"
        ? null
        : new Set(selectedInterfaceIds);
    const networkEndpoints = interfaces
      .filter((networkInterface) => selectedIds === null || selectedIds.has(networkInterface.id))
      .flatMap((networkInterface) =>
        networkInterface.addresses.map((address) => endpointForNetworkAddress(address, settings.port)),
      );
    return includesLoopback
      ? [`http://127.0.0.1:${settings.port}`, ...networkEndpoints]
      : networkEndpoints;
  }, [includesLoopback, interfaces, selectedInterfaceIds, settings.bindTarget.kind, settings.port]);
  const endpoints = status?.endpoints.length ? status.endpoints : predictedEndpoints;
  const selectedInterfaceUnavailable =
    isSelectedInterfacesTarget &&
    selectedInterfaceIds.some(
      (adapterId) => !interfaces.some((networkInterface) => networkInterface.id === adapterId),
    );

  useEffect(() => {
    if (!settings.enabled) {
      return;
    }
    let active = true;
    void listLocalApiNetworkInterfaces()
      .then((result) => {
        if (!active) {
          return;
        }
        setInterfaces(result);
        setInterfacesUnavailable(false);
      })
      .catch(() => {
        if (active) {
          setInterfacesUnavailable(true);
        }
      });
    void getLocalApiStatus()
      .then((result) => {
        if (active) {
          setStatus(result);
        }
      })
      .catch(() => {
        if (active) {
          setStatus(null);
        }
      });
    return () => {
      active = false;
    };
  }, [refreshKey, savedSettings, settings.enabled]);

  useEffect(() => {
    onTokenRequirementChange?.(tokenRequiredBeforeSaving);
  }, [onTokenRequirementChange, tokenRequiredBeforeSaving]);

  const updateSettings = (patch: Partial<LocalApiSettingsValue>) => {
    onChange({ ...settings, ...patch });
  };

  const copyToken = async () => {
    setIsSavingToken(true);
    setTokenMessage(null);
    try {
      const result = await getLocalApiAccessToken();
      if (!navigator.clipboard?.writeText) {
        throw new Error("Clipboard access is unavailable");
      }
      await navigator.clipboard.writeText(result.token);
      setTokenMessage(t.localApi.tokenCopied);
    } catch {
      setTokenMessage(t.localApi.tokenCopyFailed);
    } finally {
      setIsSavingToken(false);
    }
  };

  const beginTokenReplacement = () => {
    setToken("");
    setTokenMessage(null);
    setIsReplacingToken(true);
  };

  const saveToken = async (generate: boolean) => {
    const tokenToSave = token.trim();
    if (!generate && !tokenToSave) {
      setTokenMessage(t.localApi.tokenRequiredBeforeSave);
      return;
    }
    setIsSavingToken(true);
    setTokenMessage(null);
    try {
      await setLocalApiAccessToken(generate ? undefined : tokenToSave);
      setToken("");
      setIsReplacingToken(false);
      setTokenMessage(t.localApi.tokenSaved);
      setStatus(await getLocalApiStatus());
    } catch {
      setTokenMessage(t.localApi.tokenSaveFailed);
    } finally {
      setIsSavingToken(false);
    }
  };

  const updateSelectedInterfaces = (adapterId: string, checked: boolean) => {
    const nextAdapterIds = checked
      ? [...selectedInterfaceIds, adapterId]
      : selectedInterfaceIds.filter((id) => id !== adapterId);
    updateSettings({
      bindTarget: {
        kind: "network-interfaces",
        adapterIds: nextAdapterIds,
        includeLoopback: includesLoopback,
      },
    });
  };

  const updateIncludeLoopback = (includeLoopback: boolean) => {
    switch (settings.bindTarget.kind) {
      case "network-interface":
        updateSettings({
          bindTarget: {
            kind: "network-interface",
            adapterId: settings.bindTarget.adapterId,
            includeLoopback,
          },
        });
        break;
      case "network-interfaces":
        updateSettings({
          bindTarget: {
            kind: "network-interfaces",
            adapterIds: selectedInterfaceIds,
            includeLoopback,
          },
        });
        break;
      case "all-network-interfaces":
        updateSettings({
          bindTarget: { kind: "all-network-interfaces", includeLoopback },
        });
        break;
      default:
        break;
    }
  };

  const toggle = (
    <label className="checkbox-row settings-toggle-row">
      <input
        type="checkbox"
        data-testid="local-api-enabled"
        checked={settings.enabled}
        onChange={(event) => updateSettings({ enabled: event.currentTarget.checked })}
      />
      {t.localApi.enabled}
    </label>
  );

  if (!settings.enabled) {
    return toggle;
  }

  return (
    <section className="local-api-settings" aria-label={t.localApi.title}>
      <div className="settings-section-title">
        <h3>{t.localApi.title}</h3>
        <span>
          {status?.error
            ? t.localApi.unavailable
            : status?.running
              ? t.localApi.running
              : t.localApi.stopped}
        </span>
      </div>
      {toggle}
      <p className="settings-hint">{t.localApi.enabledHint}</p>
      <div className="settings-grid local-api-settings__grid">
        <label>
          {t.localApi.listenTarget}
          <select
            data-testid="local-api-bind-target"
            value={
              settings.bindTarget.kind === "loopback"
                ? "loopback"
                : settings.bindTarget.kind === "all-network-interfaces"
                  ? "all-network-interfaces"
                  : "network-interfaces"
            }
            onChange={(event) => {
              switch (event.currentTarget.value) {
                case "loopback":
                  updateSettings({ bindTarget: { kind: "loopback" } });
                  break;
                case "all-network-interfaces":
                  updateSettings({
                    bindTarget: { kind: "all-network-interfaces", includeLoopback: includesLoopback },
                  });
                  break;
                default:
                  updateSettings({
                    bindTarget: {
                      kind: "network-interfaces",
                      adapterIds: selectedInterfaceIds,
                      includeLoopback: includesLoopback,
                    },
                  });
              }
            }}
          >
            <option value="loopback">{t.localApi.loopback}</option>
            <option value="network-interfaces">{t.localApi.selectedNetworkInterfaces}</option>
            <option value="all-network-interfaces">{t.localApi.allNetworkInterfaces}</option>
          </select>
        </label>
        <label>
          {t.localApi.port}
          <input
            type="number"
            min={1}
            max={65535}
            data-testid="local-api-port"
            value={settings.port}
            onChange={(event) => updateSettings({ port: Number(event.currentTarget.value) })}
          />
          {!portIsValid ? <span className="field-error">{t.localApi.portError}</span> : null}
        </label>
      </div>
      {isNetworkTarget ? (
        <label className="checkbox-row">
          <input
            type="checkbox"
            data-testid="local-api-include-loopback"
            checked={includesLoopback}
            onChange={(event) => updateIncludeLoopback(event.currentTarget.checked)}
          />
          <span>
            {t.localApi.includeLoopback}
            <small className="settings-hint">{t.localApi.includeLoopbackHint}</small>
          </span>
        </label>
      ) : null}
      {isSelectedInterfacesTarget ? (
        <div className="local-api-settings__interfaces">
          <p className="settings-hint">{t.localApi.selectedInterfacesHint}</p>
          {interfaces.map((networkInterface) => (
            <label className="checkbox-row" key={networkInterface.id}>
              <input
                type="checkbox"
                data-testid={`local-api-interface-${networkInterface.id}`}
                checked={selectedInterfaceIds.includes(networkInterface.id)}
                onChange={(event) =>
                  updateSelectedInterfaces(networkInterface.id, event.currentTarget.checked)
                }
              />
              {t.localApi.networkInterface(
                networkInterface.name,
                networkInterface.addresses.map(formatNetworkAddress).join(", "),
              )}
            </label>
          ))}
        </div>
      ) : null}
      {interfacesUnavailable ? <p className="field-error">{t.localApi.interfaceLoadFailed}</p> : null}
      {isSelectedInterfacesTarget && selectedInterfaceIds.length === 0 && !includesLoopback ? (
        <p className="field-error">{t.localApi.noInterfacesSelected}</p>
      ) : null}
      {hasNetworkListener &&
      (selectedInterfaceUnavailable ||
        (settings.bindTarget.kind === "all-network-interfaces" && interfaces.length === 0)) &&
      !interfacesUnavailable ? (
        <p className="field-error">{t.localApi.noNetworkInterfaces}</p>
      ) : null}
      <div className="local-api-settings__status">
        <span>{t.localApi.endpoint}</span>
        <div className="local-api-settings__endpoints">
          {endpoints.length > 0
            ? endpoints.map((endpoint) => <code key={endpoint}>{endpoint}</code>)
            : "—"}
        </div>
      </div>
      {status?.error ? <p className="field-error">{status.error}</p> : null}
      <div className="local-api-settings__auth">
        <strong>{t.localApi.authentication}</strong>
        {hasNetworkListener ? (
          <>
            <p className="settings-hint">{t.localApi.authenticationRequired}</p>
            {tokenEditorVisible ? (
              <>
                <label>
                  {t.localApi.token}
                  <input
                    type="password"
                    autoComplete="off"
                    data-testid="local-api-token"
                    value={token}
                    placeholder={t.localApi.tokenPlaceholder}
                    onChange={(event) => setToken(event.currentTarget.value)}
                  />
                </label>
                <div className="settings-actions settings-actions--inline">
                  {hasSavedToken ? (
                    <button
                      type="button"
                      className="button-secondary"
                      disabled={isSavingToken}
                      onClick={() => setIsReplacingToken(false)}
                    >
                      {t.settings.cancel}
                    </button>
                  ) : null}
                  <button
                    type="button"
                    className="button-secondary"
                    disabled={isSavingToken}
                    onClick={() => void saveToken(false)}
                  >
                    {t.localApi.saveToken}
                  </button>
                  <button
                    type="button"
                    className="button-danger"
                    disabled={isSavingToken}
                    onClick={() => void saveToken(true)}
                  >
                    {t.localApi.generateToken}
                  </button>
                </div>
              </>
            ) : (
              <div className="local-api-settings__saved-token" data-testid="local-api-token-configured">
                <code data-testid="local-api-token-masked" aria-label={t.localApi.tokenConfigured}>
                  ••••••••••••
                </code>
                <span>{t.localApi.tokenConfigured}</span>
                <div className="settings-actions settings-actions--inline">
                  <button
                    type="button"
                    className="button-secondary"
                    disabled={isSavingToken}
                    onClick={() => void copyToken()}
                  >
                    {t.localApi.copyToken}
                  </button>
                  <button
                    type="button"
                    className="button-secondary"
                    disabled={isSavingToken}
                    onClick={beginTokenReplacement}
                  >
                    {t.localApi.replaceToken}
                  </button>
                  <button
                    type="button"
                    className="button-danger"
                    disabled={isSavingToken}
                    onClick={() => void saveToken(true)}
                  >
                    {t.localApi.generateToken}
                  </button>
                </div>
              </div>
            )}
            <p className="settings-hint">{t.localApi.tokenHint}</p>
            {tokenRequiredBeforeSaving ? (
              <p className="field-error">{t.localApi.tokenRequiredBeforeSave}</p>
            ) : null}
            <p className="local-api-settings__warning">{t.localApi.networkWarning}</p>
            {tokenMessage ? <p className="settings-message" role="status">{tokenMessage}</p> : null}
          </>
        ) : (
          <p className="settings-hint">{t.localApi.localAuthenticationNotRequired}</p>
        )}
      </div>
    </section>
  );
}
