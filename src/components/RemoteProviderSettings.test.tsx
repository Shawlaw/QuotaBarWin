import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, test, vi } from "vitest";
import type { ReactElement } from "react";
import { RemoteProviderSettings } from "./RemoteProviderSettings";
import type { RegistryInstallResult } from "../lib/api";
import { I18nProvider } from "../i18n";

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

function renderWithEnglish(ui: ReactElement) {
  return render(ui, {
    wrapper: ({ children }) => (
      <I18nProvider language="en">{children}</I18nProvider>
    )
  });
}

describe("RemoteProviderSettings", () => {
  test("open_guide_button_calls_handler", () => {
    const onOpenGuide = vi.fn();
    renderWithEnglish(
      <RemoteProviderSettings
        registrySettings={registrySettings}
        onRegistrySettingsChange={vi.fn()}
        onInstallRegistry={vi.fn(async () => emptyResult)}
        onOpenGuide={onOpenGuide}
      />
    );

    fireEvent.click(screen.getByTestId("open-remote-provider-guide"));
    expect(onOpenGuide).toHaveBeenCalledTimes(1);
  });

  test("renders_registry_form", () => {
    renderWithEnglish(
      <RemoteProviderSettings
        registrySettings={registrySettings}
        onRegistrySettingsChange={vi.fn()}
        onInstallRegistry={vi.fn(async () => emptyResult)}
        onOpenGuide={vi.fn()}
      />
    );

    expect(screen.getByText("Remote Sources")).toBeInTheDocument();
    expect(screen.getByTestId("remote-provider-url-input")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Install Registry" })).toBeInTheDocument();
  });

  test("install_registry_button_calls_handler_with_form_values", async () => {
    const onInstallRegistry = vi.fn(async (): Promise<RegistryInstallResult> => emptyResult);
    const onRegistrySettingsChange = vi.fn();
    const { rerender } = renderWithEnglish(
      <RemoteProviderSettings
        registrySettings={registrySettings}
        onRegistrySettingsChange={onRegistrySettingsChange}
        onInstallRegistry={onInstallRegistry}
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
        registrySettings={{
          registryUrl: "https://example.com/registry.json",
          providerProxyUrl: "http://proxy:8080",
          autoUpdate: true
        }}
        onRegistrySettingsChange={onRegistrySettingsChange}
        onInstallRegistry={onInstallRegistry}
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
    renderWithEnglish(
      <RemoteProviderSettings
        registrySettings={{
          registryUrl: "https://example.com/registry.json",
          providerProxyUrl: "http://proxy:8080",
          autoUpdate: false
        }}
        onRegistrySettingsChange={vi.fn()}
        onInstallRegistry={vi.fn(async () => emptyResult)}
        onOpenGuide={vi.fn()}
      />
    );

    expect(screen.getByTestId("remote-provider-url-input")).toHaveValue("https://example.com/registry.json");
    expect(screen.getByTestId("remote-provider-proxy-url-input")).toHaveValue("http://proxy:8080");
    expect(screen.getByRole("checkbox")).not.toBeChecked();
  });
});
