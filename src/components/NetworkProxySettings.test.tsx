import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, test, vi } from "vitest";
import { NetworkProxySettings } from "./NetworkProxySettings";
import type { ProxyConfig } from "../types";

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

    expect(screen.getByTestId("proxy-kind-select")).toHaveValue("http");
    expect(screen.getByTestId("proxy-url-input")).toHaveValue("http://proxy:8080");
  });

  test("changing_kind_to_http_emits_config_with_existing_url", () => {
    const onChange = vi.fn();
    render(<NetworkProxySettings proxy={null} onChange={onChange} />);

    fireEvent.change(screen.getByTestId("proxy-kind-select"), { target: { value: "http" } });

    expect(onChange).toHaveBeenCalledWith<ProxyConfig[]>({ kind: "http", url: "" });
  });

  test("changing_url_emits_updated_config", () => {
    const onChange = vi.fn();
    render(<NetworkProxySettings proxy={{ kind: "http", url: "" }} onChange={onChange} />);

    fireEvent.change(screen.getByTestId("proxy-url-input"), { target: { value: "http://proxy:8080" } });

    expect(onChange).toHaveBeenCalledWith<ProxyConfig[]>({ kind: "http", url: "http://proxy:8080" });
  });

  test("selecting_none_clears_proxy", () => {
    const onChange = vi.fn();
    render(<NetworkProxySettings proxy={{ kind: "http", url: "http://proxy:8080" }} onChange={onChange} />);

    fireEvent.change(screen.getByTestId("proxy-kind-select"), { target: { value: "none" } });

    expect(onChange).toHaveBeenCalledWith(null);
  });
});
