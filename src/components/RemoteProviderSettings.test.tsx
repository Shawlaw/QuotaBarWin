import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { describe, expect, test, vi } from "vitest";
import { RemoteProviderSettings } from "./RemoteProviderSettings";
import type { RemoteProviderConfig } from "../types";
import type { RegistryInstallResult, UpdateInfo } from "../lib/api";

const remoteProvider: RemoteProviderConfig = {
  id: "remote-kimi",
  name: "Remote Kimi",
  enabled: true,
  kind: "remote",
  manifestUrl: "https://example.com/provider.json",
  sourceUrl: "https://example.com/provider.cjs",
  runtime: "node",
  autoUpdate: true,
  updateIntervalSeconds: 3600
};

const emptyResult: RegistryInstallResult = {
  installed: [],
  skipped: [],
  failed: []
};

const registrySettings = {
  registryUrl: "",
  providerProxyUrl: "",
  autoUpdate: true
};

describe("RemoteProviderSettings", () => {
  test("open_guide_button_calls_handler", () => {
    const onOpenGuide = vi.fn();
    render(
      <RemoteProviderSettings
        providers={[]}
        registrySettings={registrySettings}
        onRegistrySettingsChange={vi.fn()}
        onInstallRegistry={vi.fn(async () => emptyResult)}
        onRemove={vi.fn()}
        onRefresh={vi.fn()}
        onCheckUpdates={vi.fn()}
        onApplyUpdate={vi.fn()}
        onOpenGuide={onOpenGuide}
      />
    );

    fireEvent.click(screen.getByTestId("open-remote-provider-guide"));
    expect(onOpenGuide).toHaveBeenCalledTimes(1);
  });

  test("renders_add_form_and_empty_state", () => {
    render(
      <RemoteProviderSettings
        providers={[]}
        registrySettings={registrySettings}
        onRegistrySettingsChange={vi.fn()}
        onInstallRegistry={vi.fn(async () => emptyResult)}
        onRemove={vi.fn()}
        onRefresh={vi.fn()}
        onCheckUpdates={vi.fn()}
        onApplyUpdate={vi.fn()}
        onOpenGuide={vi.fn()}
      />
    );

    expect(screen.getByText("Remote Sources")).toBeInTheDocument();
    expect(screen.getByTestId("remote-provider-url-input")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Install Registry" })).toBeInTheDocument();
    expect(screen.getByText("No remote providers installed")).toBeInTheDocument();
  });

  test("install_registry_button_calls_handler_with_form_values", async () => {
    const onInstallRegistry = vi.fn(async (): Promise<RegistryInstallResult> => emptyResult);
    const onRegistrySettingsChange = vi.fn();
    const { rerender } = render(
      <RemoteProviderSettings
        providers={[]}
        registrySettings={registrySettings}
        onRegistrySettingsChange={onRegistrySettingsChange}
        onInstallRegistry={onInstallRegistry}
        onRemove={vi.fn()}
        onRefresh={vi.fn()}
        onCheckUpdates={vi.fn()}
        onApplyUpdate={vi.fn()}
        onOpenGuide={vi.fn()}
      />
    );

    fireEvent.change(screen.getByTestId("remote-provider-url-input"), {
      target: { value: "https://example.com/registry.json" }
    });

    expect(onRegistrySettingsChange).toHaveBeenLastCalledWith({
      registryUrl: "https://example.com/registry.json",
      providerProxyUrl: "",
      autoUpdate: true
    });

    rerender(
      <RemoteProviderSettings
        providers={[]}
        registrySettings={{
          registryUrl: "https://example.com/registry.json",
          providerProxyUrl: "",
          autoUpdate: true
        }}
        onRegistrySettingsChange={onRegistrySettingsChange}
        onInstallRegistry={onInstallRegistry}
        onRemove={vi.fn()}
        onRefresh={vi.fn()}
        onCheckUpdates={vi.fn()}
        onApplyUpdate={vi.fn()}
        onOpenGuide={vi.fn()}
      />
    );

    fireEvent.change(screen.getByTestId("remote-provider-proxy-url-input"), {
      target: { value: "http://proxy:8080" }
    });

    rerender(
      <RemoteProviderSettings
        providers={[]}
        registrySettings={{
          registryUrl: "https://example.com/registry.json",
          providerProxyUrl: "http://proxy:8080",
          autoUpdate: true
        }}
        onRegistrySettingsChange={onRegistrySettingsChange}
        onInstallRegistry={onInstallRegistry}
        onRemove={vi.fn()}
        onRefresh={vi.fn()}
        onCheckUpdates={vi.fn()}
        onApplyUpdate={vi.fn()}
        onOpenGuide={vi.fn()}
      />
    );

    fireEvent.click(screen.getByRole("button", { name: "Install Registry" }));

    expect(onInstallRegistry).toHaveBeenCalledWith(
      "https://example.com/registry.json",
      "http://proxy:8080",
      true
    );
    await screen.findByText("No providers installed from registry");
  });

  test("renders_persisted_registry_settings", () => {
    render(
      <RemoteProviderSettings
        providers={[]}
        registrySettings={{
          registryUrl: "https://example.com/registry.json",
          providerProxyUrl: "http://proxy:8080",
          autoUpdate: false
        }}
        onRegistrySettingsChange={vi.fn()}
        onInstallRegistry={vi.fn(async () => emptyResult)}
        onRemove={vi.fn()}
        onRefresh={vi.fn()}
        onCheckUpdates={vi.fn()}
        onApplyUpdate={vi.fn()}
        onOpenGuide={vi.fn()}
      />
    );

    expect(screen.getByTestId("remote-provider-url-input")).toHaveValue("https://example.com/registry.json");
    expect(screen.getByTestId("remote-provider-proxy-url-input")).toHaveValue("http://proxy:8080");
    expect(screen.getByRole("checkbox")).not.toBeChecked();
  });

  test("lists_installed_remote_providers", () => {
    render(
      <RemoteProviderSettings
        providers={[remoteProvider]}
        registrySettings={registrySettings}
        onRegistrySettingsChange={vi.fn()}
        onInstallRegistry={vi.fn(async () => emptyResult)}
        onRemove={vi.fn()}
        onRefresh={vi.fn()}
        onCheckUpdates={vi.fn()}
        onApplyUpdate={vi.fn()}
        onOpenGuide={vi.fn()}
      />
    );

    expect(screen.getByText("Remote Kimi")).toBeInTheDocument();
    expect(screen.getByText("remote-kimi")).toBeInTheDocument();
    expect(screen.getByText("node")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Details" })).toBeInTheDocument();
    expect(screen.queryByText("https://example.com/provider.json")).not.toBeInTheDocument();
  });

  test("toggles_installed_provider_details", () => {
    render(
      <RemoteProviderSettings
        providers={[remoteProvider]}
        registrySettings={registrySettings}
        onRegistrySettingsChange={vi.fn()}
        onInstallRegistry={vi.fn(async () => emptyResult)}
        onRemove={vi.fn()}
        onRefresh={vi.fn()}
        onCheckUpdates={vi.fn()}
        onApplyUpdate={vi.fn()}
        onOpenGuide={vi.fn()}
      />
    );

    fireEvent.click(screen.getByRole("button", { name: "Details" }));
    expect(screen.getByText("https://example.com/provider.json")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Remove" })).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "Collapse" }));
    expect(screen.queryByText("https://example.com/provider.json")).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Remove" })).not.toBeInTheDocument();
  });

  test("remove_button_calls_handler", async () => {
    const onRemove = vi.fn();
    render(
      <RemoteProviderSettings
        providers={[remoteProvider]}
        registrySettings={registrySettings}
        onRegistrySettingsChange={vi.fn()}
        onInstallRegistry={vi.fn(async () => emptyResult)}
        onRemove={onRemove}
        onRefresh={vi.fn()}
        onCheckUpdates={vi.fn()}
        onApplyUpdate={vi.fn()}
        onOpenGuide={vi.fn()}
      />
    );

    fireEvent.click(screen.getByRole("button", { name: "Details" }));
    fireEvent.click(screen.getByRole("button", { name: "Remove" }));
    expect(onRemove).toHaveBeenCalledWith("remote-kimi");
    await screen.findByText("Provider removed");
  });

  test("check_updates_button_displays_available_updates", async () => {
    const update: UpdateInfo = { id: "remote-kimi", available: true, newChecksum: "sha256:new" };
    const onCheckUpdates = vi.fn(async () => [update]);
    render(
      <RemoteProviderSettings
        providers={[remoteProvider]}
        registrySettings={registrySettings}
        onRegistrySettingsChange={vi.fn()}
        onInstallRegistry={vi.fn(async () => emptyResult)}
        onRemove={vi.fn()}
        onRefresh={vi.fn()}
        onCheckUpdates={onCheckUpdates}
        onApplyUpdate={vi.fn()}
        onOpenGuide={vi.fn()}
      />
    );

    fireEvent.click(screen.getByRole("button", { name: "Details" }));
    fireEvent.click(screen.getByRole("button", { name: "Check Updates" }));
    await screen.findByText("1 update(s) available");
    expect(onCheckUpdates).toHaveBeenCalled();
  });

  test("apply_update_button_calls_handler", async () => {
    const onApplyUpdate = vi.fn();
    render(
      <RemoteProviderSettings
        providers={[remoteProvider]}
        registrySettings={registrySettings}
        onRegistrySettingsChange={vi.fn()}
        onInstallRegistry={vi.fn(async () => emptyResult)}
        onRemove={vi.fn()}
        onRefresh={vi.fn()}
        onCheckUpdates={vi.fn(async () => [
          { id: "remote-kimi", available: true, newChecksum: "sha256:new" }
        ])}
        onApplyUpdate={onApplyUpdate}
        onOpenGuide={vi.fn()}
      />
    );

    fireEvent.click(screen.getByRole("button", { name: "Details" }));
    fireEvent.click(screen.getByRole("button", { name: "Check Updates" }));
    const applyButton = await screen.findByRole("button", { name: "Apply Update" });
    fireEvent.click(applyButton);
    await waitFor(() => expect(onApplyUpdate).toHaveBeenCalledWith("remote-kimi"));
    await screen.findByText("Update applied");
  });
});
