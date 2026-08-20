import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, test, vi } from "vitest";
import { LocalApiSettings } from "./LocalApiSettings";
import { I18nProvider } from "../i18n";
import {
  getLocalApiAccessToken,
  getLocalApiStatus,
  listLocalApiNetworkInterfaces,
  setLocalApiAccessToken,
} from "../lib/api";

vi.mock("../lib/api", () => ({
  getLocalApiAccessToken: vi.fn(),
  getLocalApiStatus: vi.fn(),
  listLocalApiNetworkInterfaces: vi.fn(),
  setLocalApiAccessToken: vi.fn(),
}));

const getLocalApiAccessTokenMock = vi.mocked(getLocalApiAccessToken);
const getLocalApiStatusMock = vi.mocked(getLocalApiStatus);
const listLocalApiNetworkInterfacesMock = vi.mocked(listLocalApiNetworkInterfaces);
const setLocalApiAccessTokenMock = vi.mocked(setLocalApiAccessToken);

const loopbackSettings = {
  enabled: true,
  bindTarget: { kind: "loopback" as const },
  port: 41833,
};

afterEach(() => {
  getLocalApiAccessTokenMock.mockReset();
  getLocalApiStatusMock.mockReset();
  listLocalApiNetworkInterfacesMock.mockReset();
  setLocalApiAccessTokenMock.mockReset();
});

function renderSettings(onChange = vi.fn()) {
  listLocalApiNetworkInterfacesMock.mockResolvedValue([
    {
      id: "adapter-1",
      name: "Ethernet",
      addresses: [{ address: "192.168.1.10", scopeId: null }],
      isPrivate: true,
    },
    {
      id: "adapter-2",
      name: "Wi-Fi",
      addresses: [{ address: "192.168.50.10", scopeId: null }],
      isPrivate: true,
    },
  ]);
  getLocalApiStatusMock.mockResolvedValue({
    enabled: true,
    running: true,
    endpoints: ["http://127.0.0.1:41833"],
    requiresAuth: false,
    tokenConfigured: false,
    error: null,
  });

  render(
    <I18nProvider language="en">
      <LocalApiSettings settings={loopbackSettings} onChange={onChange} />
    </I18nProvider>,
  );
  return onChange;
}

describe("LocalApiSettings", () => {
  test("keeps_only_the_enable_toggle_visible_while_disabled", () => {
    render(
      <I18nProvider language="en">
        <LocalApiSettings
          settings={{ ...loopbackSettings, enabled: false }}
          onChange={vi.fn()}
        />
      </I18nProvider>,
    );

    expect(screen.getByTestId("local-api-enabled")).not.toBeChecked();
    expect(screen.queryByText("Listen on")).not.toBeInTheDocument();
    expect(listLocalApiNetworkInterfacesMock).not.toHaveBeenCalled();
    expect(getLocalApiStatusMock).not.toHaveBeenCalled();
  });

  test("defaults_to_loopback_and_offers_selected_or_all_network_interfaces", async () => {
    renderSettings();

    await waitFor(() => {
      expect(screen.getByTestId("local-api-bind-target")).toHaveValue("loopback");
    });
    expect(screen.getByRole("option", { name: "Selected network interfaces" })).toBeInTheDocument();
    expect(screen.getByRole("option", { name: "All active network interfaces" })).toBeInTheDocument();
  });

  test("selecting_multiple_interfaces_emits_a_multi_interface_target", async () => {
    const onChange = renderSettings();

    await waitFor(() => expect(screen.getByTestId("local-api-bind-target")).toBeInTheDocument());
    fireEvent.change(screen.getByTestId("local-api-bind-target"), {
      target: { value: "network-interfaces" },
    });

    expect(onChange).toHaveBeenCalledWith({
      enabled: true,
      bindTarget: { kind: "network-interfaces", adapterIds: [], includeLoopback: true },
      port: 41833,
    });
  });

  test("selecting_all_interfaces_emits_an_all_interfaces_target", async () => {
    const onChange = renderSettings();

    await waitFor(() => expect(screen.getByTestId("local-api-bind-target")).toBeInTheDocument());
    fireEvent.change(screen.getByTestId("local-api-bind-target"), {
      target: { value: "all-network-interfaces" },
    });

    expect(onChange).toHaveBeenCalledWith({
      enabled: true,
      bindTarget: { kind: "all-network-interfaces", includeLoopback: true },
      port: 41833,
    });
  });

  test("allows_a_loopback_only_selection_without_requiring_a_token", async () => {
    listLocalApiNetworkInterfacesMock.mockResolvedValue([]);
    getLocalApiStatusMock.mockResolvedValue({
      enabled: true,
      running: true,
      endpoints: ["http://127.0.0.1:41833"],
      requiresAuth: false,
      tokenConfigured: false,
      error: null,
    });
    render(
      <I18nProvider language="en">
        <LocalApiSettings
          settings={{
            enabled: true,
            bindTarget: { kind: "network-interfaces", adapterIds: [], includeLoopback: true },
            port: 41833,
          }}
          onChange={vi.fn()}
        />
      </I18nProvider>,
    );

    await waitFor(() => expect(screen.getByText("Loopback access does not require a token.")).toBeInTheDocument());
    expect(screen.queryByText("Select at least one network interface.")).not.toBeInTheDocument();
    expect(screen.queryByTestId("local-api-token")).not.toBeInTheDocument();
  });

  test("formats_scoped_ipv6_endpoints_with_brackets_and_an_encoded_scope", async () => {
    listLocalApiNetworkInterfacesMock.mockResolvedValue([
      {
        id: "adapter-6",
        name: "Ethernet",
        addresses: [{ address: "fe80::1234", scopeId: 12 }],
        isPrivate: true,
      },
    ]);
    getLocalApiStatusMock.mockResolvedValue({
      enabled: true,
      running: false,
      endpoints: [],
      requiresAuth: true,
      tokenConfigured: true,
      error: null,
    });
    render(
      <I18nProvider language="en">
        <LocalApiSettings
          settings={{
            enabled: true,
            bindTarget: {
              kind: "network-interfaces",
              adapterIds: ["adapter-6"],
              includeLoopback: false,
            },
            port: 41833,
          }}
          onChange={vi.fn()}
        />
      </I18nProvider>,
    );

    await waitFor(() => {
      expect(screen.getByText("http://[fe80::1234%2512]:41833")).toBeInTheDocument();
    });
  });

  test("adding_an_interface_to_a_selected_target_preserves_existing_selections", async () => {
    const onChange = vi.fn();
    listLocalApiNetworkInterfacesMock.mockResolvedValue([
      {
        id: "adapter-1",
        name: "Ethernet",
        addresses: [{ address: "192.168.1.10", scopeId: null }],
        isPrivate: true,
      },
      {
        id: "adapter-2",
        name: "Wi-Fi",
        addresses: [{ address: "192.168.50.10", scopeId: null }],
        isPrivate: true,
      },
    ]);
    getLocalApiStatusMock.mockResolvedValue({
      enabled: true,
      running: true,
      endpoints: ["http://192.168.1.10:41833"],
      requiresAuth: true,
      tokenConfigured: true,
      error: null,
    });
    render(
      <I18nProvider language="en">
        <LocalApiSettings
          settings={{
            enabled: true,
            bindTarget: { kind: "network-interfaces", adapterIds: ["adapter-1"] },
            port: 41833,
          }}
          onChange={onChange}
        />
      </I18nProvider>,
    );

    await waitFor(() => expect(screen.getByTestId("local-api-interface-adapter-1")).toBeChecked());
    fireEvent.click(screen.getByTestId("local-api-interface-adapter-2"));

    expect(onChange).toHaveBeenCalledWith({
      enabled: true,
      bindTarget: {
        kind: "network-interfaces",
        adapterIds: ["adapter-1", "adapter-2"],
        includeLoopback: true,
      },
      port: 41833,
    });
  });

  test("keeps_loopback_token_free_access_enabled_by_default_for_network_targets", async () => {
    const onChange = vi.fn();
    listLocalApiNetworkInterfacesMock.mockResolvedValue([]);
    getLocalApiStatusMock.mockResolvedValue({
      enabled: true,
      running: true,
      endpoints: ["http://127.0.0.1:41833", "http://192.168.1.10:41833"],
      requiresAuth: true,
      tokenConfigured: true,
      error: null,
    });
    render(
      <I18nProvider language="en">
        <LocalApiSettings
          settings={{
            enabled: true,
            bindTarget: { kind: "all-network-interfaces", includeLoopback: true },
            port: 41833,
          }}
          onChange={onChange}
        />
      </I18nProvider>,
    );

    await waitFor(() => expect(screen.getByTestId("local-api-include-loopback")).toBeChecked());
    fireEvent.click(screen.getByTestId("local-api-include-loopback"));

    expect(onChange).toHaveBeenCalledWith({
      enabled: true,
      bindTarget: { kind: "all-network-interfaces", includeLoopback: false },
      port: 41833,
    });
  });

  test("requires_a_saved_token_before_external_listener_settings_can_be_saved", async () => {
    const onTokenRequirementChange = vi.fn();
    listLocalApiNetworkInterfacesMock.mockResolvedValue([]);
    getLocalApiStatusMock.mockResolvedValue({
      enabled: true,
      running: false,
      endpoints: [],
      requiresAuth: true,
      tokenConfigured: false,
      error: null,
    });
    render(
      <I18nProvider language="en">
        <LocalApiSettings
          settings={{
            enabled: true,
            bindTarget: { kind: "all-network-interfaces", includeLoopback: true },
            port: 41833,
          }}
          onChange={vi.fn()}
          onTokenRequirementChange={onTokenRequirementChange}
        />
      </I18nProvider>,
    );

    await waitFor(() => {
      expect(
        screen.getByText("Save an access token before saving network listener settings."),
      ).toBeInTheDocument();
    });
    expect(onTokenRequirementChange).toHaveBeenLastCalledWith(true);
    fireEvent.click(screen.getByRole("button", { name: "Save token" }));
    expect(setLocalApiAccessTokenMock).not.toHaveBeenCalled();
  });

  test("manually_saves_or_regenerates_an_external_access_token", async () => {
    const onChange = vi.fn();
    const writeText = vi.fn(async () => undefined);
    Object.defineProperty(navigator, "clipboard", {
      configurable: true,
      value: { writeText },
    });
    listLocalApiNetworkInterfacesMock.mockResolvedValue([]);
    getLocalApiStatusMock.mockResolvedValue({
      enabled: true,
      running: true,
      endpoints: ["http://192.168.1.10:41833"],
      requiresAuth: true,
      tokenConfigured: true,
      error: null,
    });
    getLocalApiAccessTokenMock.mockResolvedValue({ token: "x".repeat(32) });
    setLocalApiAccessTokenMock.mockResolvedValue({ token: "x".repeat(32) });

    render(
      <I18nProvider language="en">
        <LocalApiSettings
          settings={{
            enabled: true,
            bindTarget: { kind: "network-interfaces", adapterIds: ["adapter-1"] },
            port: 41833,
          }}
          onChange={onChange}
        />
      </I18nProvider>,
    );

    await waitFor(() => {
      expect(screen.getByTestId("local-api-token-configured")).toHaveTextContent(
        "An access token is saved and shown in masked form.",
      );
    });
    expect(screen.getByTestId("local-api-token-masked")).toHaveTextContent("••••••••••••");
    expect(screen.queryByTestId("local-api-token")).not.toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "Copy token" }));
    await waitFor(() => {
      expect(writeText).toHaveBeenCalledWith("x".repeat(32));
    });

    fireEvent.click(screen.getByRole("button", { name: "Replace token" }));
    expect(screen.getByTestId("local-api-token")).toBeInTheDocument();
    fireEvent.change(screen.getByTestId("local-api-token"), { target: { value: "m".repeat(32) } });
    fireEvent.click(screen.getByRole("button", { name: "Save token" }));
    await waitFor(() => {
      expect(setLocalApiAccessTokenMock).toHaveBeenCalledWith("m".repeat(32));
    });

    fireEvent.click(screen.getByRole("button", { name: "Generate new token" }));
    await waitFor(() => {
      expect(setLocalApiAccessTokenMock).toHaveBeenCalledWith(undefined);
    });
  });
});
