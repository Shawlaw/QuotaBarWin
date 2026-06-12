import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, test, vi } from "vitest";
import { RemoteProviderSettings } from "./RemoteProviderSettings";
import type { RemoteProviderConfig } from "../types";
import type { RemoteProviderPreview, UpdateInfo } from "../lib/api";

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

const preview: RemoteProviderPreview = {
  id: "remote-kimi",
  name: "Remote Kimi",
  runtime: "node",
  sourceUrl: "https://example.com/provider.cjs",
  requiredEnvVars: ["KIMI_API_KEY"],
  checksum: "sha256:abc"
};

describe("RemoteProviderSettings", () => {
  test("renders_add_form_and_empty_state", () => {
    render(
      <RemoteProviderSettings
        providers={[]}
        onPreview={vi.fn()}
        onAdd={vi.fn()}
        onRemove={vi.fn()}
        onRefresh={vi.fn()}
        onCheckUpdates={vi.fn()}
        onApplyUpdate={vi.fn()}
      />
    );

    expect(screen.getByText("Remote Providers")).toBeInTheDocument();
    expect(screen.getByTestId("remote-provider-url-input")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Preview" })).toBeInTheDocument();
    expect(screen.getByText("No remote providers installed")).toBeInTheDocument();
  });

  test("preview_button_calls_handler_with_form_values", () => {
    const onPreview = vi.fn(async () => preview);
    render(
      <RemoteProviderSettings
        providers={[]}
        onPreview={onPreview}
        onAdd={vi.fn()}
        onRemove={vi.fn()}
        onRefresh={vi.fn()}
        onCheckUpdates={vi.fn()}
        onApplyUpdate={vi.fn()}
      />
    );

    fireEvent.change(screen.getByTestId("remote-provider-url-input"), {
      target: { value: "https://example.com/provider.json" }
    });
    fireEvent.change(screen.getByTestId("remote-provider-proxy-url-input"), {
      target: { value: "http://proxy:8080" }
    });
    fireEvent.change(screen.getByRole("checkbox"), { target: { checked: true } });
    fireEvent.click(screen.getByRole("button", { name: "Preview" }));

    expect(onPreview).toHaveBeenCalledWith(
      "https://example.com/provider.json",
      "http://proxy:8080",
      true
    );
  });

  test("lists_installed_remote_providers", () => {
    render(
      <RemoteProviderSettings
        providers={[remoteProvider]}
        onPreview={vi.fn()}
        onAdd={vi.fn()}
        onRemove={vi.fn()}
        onRefresh={vi.fn()}
        onCheckUpdates={vi.fn()}
        onApplyUpdate={vi.fn()}
      />
    );

    expect(screen.getByText("Remote Kimi")).toBeInTheDocument();
    expect(screen.getByText("remote-kimi")).toBeInTheDocument();
    expect(screen.getByText("node")).toBeInTheDocument();
  });

  test("remove_button_calls_handler", () => {
    const onRemove = vi.fn();
    render(
      <RemoteProviderSettings
        providers={[remoteProvider]}
        onPreview={vi.fn()}
        onAdd={vi.fn()}
        onRemove={onRemove}
        onRefresh={vi.fn()}
        onCheckUpdates={vi.fn()}
        onApplyUpdate={vi.fn()}
      />
    );

    fireEvent.click(screen.getByRole("button", { name: "Remove" }));
    expect(onRemove).toHaveBeenCalledWith("remote-kimi");
  });

  test("check_updates_button_displays_available_updates", async () => {
    const update: UpdateInfo = { id: "remote-kimi", available: true, newChecksum: "sha256:new" };
    const onCheckUpdates = vi.fn(async () => [update]);
    render(
      <RemoteProviderSettings
        providers={[remoteProvider]}
        onPreview={vi.fn()}
        onAdd={vi.fn()}
        onRemove={vi.fn()}
        onRefresh={vi.fn()}
        onCheckUpdates={onCheckUpdates}
        onApplyUpdate={vi.fn()}
      />
    );

    fireEvent.click(screen.getByRole("button", { name: "Check Updates" }));
    await screen.findByText("1 update(s) available");
    expect(onCheckUpdates).toHaveBeenCalled();
  });

  test("apply_update_button_calls_handler", async () => {
    const onApplyUpdate = vi.fn();
    render(
      <RemoteProviderSettings
        providers={[remoteProvider]}
        onPreview={vi.fn()}
        onAdd={vi.fn()}
        onRemove={vi.fn()}
        onRefresh={vi.fn()}
        onCheckUpdates={vi.fn(async () => [
          { id: "remote-kimi", available: true, newChecksum: "sha256:new" }
        ])}
        onApplyUpdate={onApplyUpdate}
      />
    );

    fireEvent.click(screen.getByRole("button", { name: "Check Updates" }));
    const applyButton = await screen.findByRole("button", { name: "Apply Update" });
    fireEvent.click(applyButton);
    expect(onApplyUpdate).toHaveBeenCalledWith("remote-kimi");
  });
});
