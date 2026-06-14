import type { ProxyConfig, ProxyKind } from "../types";
import { useI18n } from "../i18n";
import type { I18nCatalog } from "../i18n";

type NetworkProxySettingsProps = {
  proxy: ProxyConfig | null | undefined;
  onChange: (proxy: ProxyConfig | null) => void;
};

const proxyKinds: { value: ProxyKind; labelKey: keyof I18nCatalog["networkProxy"]; needsUrl: boolean }[] = [
  { value: "none", labelKey: "noProxy", needsUrl: false },
  { value: "system", labelKey: "systemProxy", needsUrl: false },
  { value: "http", labelKey: "httpProxy", needsUrl: true },
  { value: "socks5", labelKey: "socks5Proxy", needsUrl: true }
];

export function NetworkProxySettings({ proxy, onChange }: NetworkProxySettingsProps) {
  const { t } = useI18n();
  const kind = proxy?.kind ?? "none";
  const url = proxy?.url ?? "";
  const needsUrl = proxyKinds.find((option) => option.value === kind)?.needsUrl ?? false;

  return (
    <div className="network-proxy-settings">
      <label>
        {t.networkProxy.label}
        <select
          data-testid="proxy-kind-select"
          value={kind}
          onChange={(event) => {
            const newKind = event.currentTarget.value as ProxyKind;
            if (newKind === "none") {
              onChange(null);
            } else {
              onChange({ kind: newKind, url });
            }
          }}
        >
          {proxyKinds.map((option) => (
            <option key={option.value} value={option.value}>
              {t.networkProxy[option.labelKey]}
            </option>
          ))}
        </select>
      </label>
      {needsUrl ? (
        <label>
          {t.networkProxy.proxyUrl}
          <input
            data-testid="proxy-url-input"
            type="text"
            value={url}
            placeholder={t.networkProxy.proxyUrlPlaceholder}
            onChange={(event) => onChange({ kind, url: event.currentTarget.value })}
          />
        </label>
      ) : null}
    </div>
  );
}
