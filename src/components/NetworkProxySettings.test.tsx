import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { useState } from "react";
import { afterEach, describe, expect, test, vi } from "vitest";
import { NetworkProxySettings } from "./NetworkProxySettings";
import type { ProxyConfig } from "../types";
import { testNetworkProxy } from "../lib/api";

vi.mock("../lib/api", () => ({
  testNetworkProxy: vi.fn(),
}));

const testNetworkProxyMock = vi.mocked(testNetworkProxy);

afterEach(() => {
  testNetworkProxyMock.mockReset();
});

describe("NetworkProxySettings", () => {
  test("selects_none_when_no_proxy_configured", () => {
    const onChange = vi.fn();
    render(<NetworkProxySettings proxy={null} onChange={onChange} />);

    expect(screen.getByTestId("proxy-kind-select")).toHaveValue("none");
    expect(screen.queryByTestId("proxy-url-input")).not.toBeInTheDocument();
  });

  test("shows_url_input_when_proxy_requires_url", () => {
    const onChange = vi.fn();
    render(<NetworkProxySettings proxy={{ kind: "http", url: "http://proxy:8080" }} onChange={onChange} />);

    // Persisted http/socks5 kinds both show as the single custom selection.
    expect(screen.getByTestId("proxy-kind-select")).toHaveValue("custom");
    expect(screen.getByTestId("proxy-url-input")).toHaveValue("http://proxy:8080");
  });

  test("selecting_custom_derives_the_kind_from_the_existing_url", () => {
    const onChange = vi.fn();
    render(<NetworkProxySettings proxy={null} onChange={onChange} />);

    fireEvent.change(screen.getByTestId("proxy-kind-select"), { target: { value: "custom" } });
    expect(onChange).toHaveBeenCalledWith<ProxyConfig[]>({ kind: "http", url: "" });
  });

  test("changing_url_emits_updated_config", () => {
    const onChange = vi.fn();
    render(<NetworkProxySettings proxy={{ kind: "http", url: "" }} onChange={onChange} />);

    fireEvent.change(screen.getByTestId("proxy-url-input"), { target: { value: "http://proxy:8080" } });

    expect(onChange).toHaveBeenCalledWith<ProxyConfig[]>({ kind: "http", url: "http://proxy:8080" });
  });

  test("a_socks5_url_keeps_the_socks5_kind_without_a_separate_selection", () => {
    const onChange = vi.fn();
    // A stateful harness mirrors the real settings page: the component only
    // sees its updated proxy through the parent re-render.
    function Harness() {
      const [proxy, setProxy] = useState<ProxyConfig | null>(null);
      return (
        <NetworkProxySettings
          proxy={proxy}
          onChange={(next) => {
            onChange(next);
            setProxy(next);
          }}
        />
      );
    }
    render(<Harness />);

    fireEvent.change(screen.getByTestId("proxy-kind-select"), { target: { value: "custom" } });
    fireEvent.change(screen.getByTestId("proxy-url-input"), { target: { value: "socks5h://proxy:1080" } });

    expect(onChange).toHaveBeenLastCalledWith<ProxyConfig[]>({ kind: "socks5", url: "socks5h://proxy:1080" });
  });

  test("selecting_none_clears_proxy", () => {
    const onChange = vi.fn();
    render(<NetworkProxySettings proxy={{ kind: "http", url: "http://proxy:8080" }} onChange={onChange} />);

    fireEvent.change(screen.getByTestId("proxy-kind-select"), { target: { value: "none" } });

    expect(onChange).toHaveBeenCalledWith(null);
  });

  test("tests_the_current_unsaved_proxy_with_the_github_default", async () => {
    testNetworkProxyMock.mockResolvedValue({
      success: true,
      statusCode: 200,
      elapsedMs: 120,
    });
    const onChange = vi.fn();
    render(<NetworkProxySettings proxy={{ kind: "http", url: "http://proxy:8080" }} onChange={onChange} />);

    fireEvent.click(screen.getByTestId("test-proxy-button"));

    await waitFor(() => {
      expect(testNetworkProxyMock).toHaveBeenCalledWith(
        { kind: "http", url: "http://proxy:8080" },
        "https://github.com/",
      );
    });
    expect(screen.getByRole("status")).toHaveTextContent(/代理可用|Proxy is available/);
  });

  test("uses_a_custom_test_url_without_changing_the_saved_proxy", async () => {
    testNetworkProxyMock.mockResolvedValue({
      success: false,
      statusCode: null,
      elapsedMs: 0,
      errorKind: "invalidTarget",
    });
    const onChange = vi.fn();
    render(<NetworkProxySettings proxy={{ kind: "socks5", url: "socks5://proxy:1080" }} onChange={onChange} />);

    fireEvent.change(screen.getByTestId("proxy-test-url-input"), {
      target: { value: "http://insecure.example.com/" },
    });
    fireEvent.click(screen.getByTestId("test-proxy-button"));

    await waitFor(() => {
      expect(testNetworkProxyMock).toHaveBeenCalledWith(
        { kind: "socks5", url: "socks5://proxy:1080" },
        "http://insecure.example.com/",
      );
    });
    expect(onChange).not.toHaveBeenCalled();
    expect(screen.getByRole("status")).toHaveTextContent(/HTTPS/);
  });

  test("disables_proxy_testing_until_a_proxy_is_configured", () => {
    const onChange = vi.fn();
    render(<NetworkProxySettings proxy={{ kind: "http", url: "" }} onChange={onChange} />);

    expect(screen.getByTestId("test-proxy-button")).toBeDisabled();
  });
});
