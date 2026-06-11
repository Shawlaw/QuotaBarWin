import type { ProxyConfig, ProxyKind } from "../types";

type NetworkProxySettingsProps = {
  proxy: ProxyConfig | null | undefined;
  onChange: (proxy: ProxyConfig | null) => void;
};

const proxyKinds: { value: ProxyKind; label: string; needsUrl: boolean }[] = [
  { value: "none", label: "No proxy", needsUrl: false },
  { value: "system", label: "System proxy", needsUrl: false },
  { value: "http", label: "HTTP proxy", needsUrl: true },
  { value: "socks5", label: "SOCKS5 proxy", needsUrl: true }
];

export function NetworkProxySettings({ proxy, onChange }: NetworkProxySettingsProps) {
  const kind = proxy?.kind ?? "none";
  const url = proxy?.url ?? "";
  const needsUrl = proxyKinds.find((option) => option.value === kind)?.needsUrl ?? false;

  return (
    <div className="network-proxy-settings settings-grid">
      <label>
        Network proxy
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
              {option.label}
            </option>
          ))}
        </select>
      </label>
      {needsUrl ? (
        <label>
          Proxy URL
          <input
            data-testid="proxy-url-input"
            type="text"
            value={url}
            placeholder="http://host:port or socks5://host:port"
            onChange={(event) => onChange({ kind, url: event.currentTarget.value })}
          />
        </label>
      ) : null}
    </div>
  );
}
