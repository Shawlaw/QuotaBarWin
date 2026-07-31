import { useState } from "react";
import type { ProxyConfig, ProxyKind, ProxyTestResult } from "../types";
import { useI18n } from "../i18n";
import { testNetworkProxy } from "../lib/api";

type NetworkProxySettingsProps = {
  proxy: ProxyConfig | null | undefined;
  onChange: (proxy: ProxyConfig | null) => void;
};

type ProxyKindLabelKey = "noProxy" | "systemProxy" | "httpProxy" | "socks5Proxy";

const proxyKinds: { value: ProxyKind; labelKey: ProxyKindLabelKey; needsUrl: boolean }[] = [
  { value: "none", labelKey: "noProxy", needsUrl: false },
  { value: "system", labelKey: "systemProxy", needsUrl: false },
  { value: "http", labelKey: "httpProxy", needsUrl: true },
  { value: "socks5", labelKey: "socks5Proxy", needsUrl: true }
];

const DEFAULT_PROXY_TEST_URL = "https://github.com/";

export function NetworkProxySettings({ proxy, onChange }: NetworkProxySettingsProps) {
  const { t } = useI18n();
  const kind = proxy?.kind ?? "none";
  const url = proxy?.url ?? "";
  const needsUrl = proxyKinds.find((option) => option.value === kind)?.needsUrl ?? false;
  const [testUrl, setTestUrl] = useState(DEFAULT_PROXY_TEST_URL);
  const [isTesting, setIsTesting] = useState(false);
  const [testResult, setTestResult] = useState<ProxyTestResult | null>(null);
  const [testUnavailable, setTestUnavailable] = useState(false);
  const canTest = kind !== "none" && (!needsUrl || url.trim().length > 0);

  const resultMessage = testUnavailable
    ? t.networkProxy.testProxyUnavailable
    : testResult?.success
      ? t.networkProxy.testProxySuccess(testResult.statusCode ?? null, testResult.elapsedMs)
      : testResult?.errorKind === "noProxy"
        ? t.networkProxy.testProxyNoProxy
        : testResult?.errorKind === "invalidTarget"
          ? t.networkProxy.testProxyInvalidTarget
          : testResult?.errorKind === "invalidProxy"
            ? t.networkProxy.testProxyInvalidProxy
            : testResult?.errorKind === "httpStatus"
              ? t.networkProxy.testProxyHttpStatus(testResult.statusCode ?? null)
              : testResult?.errorKind === "requestFailed"
                ? t.networkProxy.testProxyRequestFailed
                : null;

  const handleTest = async () => {
    if (!canTest || isTesting) {
      return;
    }

    setIsTesting(true);
    setTestResult(null);
    setTestUnavailable(false);
    try {
      const result = await testNetworkProxy(
        { kind, url },
        testUrl.trim() || DEFAULT_PROXY_TEST_URL,
      );
      setTestResult(result);
    } catch {
      setTestUnavailable(true);
    } finally {
      setIsTesting(false);
    }
  };

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
      <div className="network-proxy-test">
        <div className="network-proxy-test__actions">
          <button
            className="button-secondary button-compact"
            data-testid="test-proxy-button"
            disabled={!canTest || isTesting}
            type="button"
            onClick={() => void handleTest()}
          >
            {isTesting ? t.networkProxy.testingProxy : t.networkProxy.testProxy}
          </button>
          <span className="network-proxy-test__hint">{t.networkProxy.testProxyHint}</span>
        </div>
        <details className="network-proxy-test__custom-target">
          <summary>{t.networkProxy.customTestUrl}</summary>
          <label>
            {t.networkProxy.testUrl}
            <input
              data-testid="proxy-test-url-input"
              type="url"
              value={testUrl}
              placeholder={t.networkProxy.testUrlPlaceholder}
              onChange={(event) => setTestUrl(event.currentTarget.value)}
            />
          </label>
        </details>
        {resultMessage ? (
          <p
            className={testResult?.success ? "network-proxy-test__result network-proxy-test__result--success" : "network-proxy-test__result"}
            role="status"
          >
            {resultMessage}
          </p>
        ) : null}
      </div>
    </div>
  );
}
