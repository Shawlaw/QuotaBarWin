import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, test, vi } from "vitest";
import { InstallRemoteProviderDialog } from "./InstallRemoteProviderDialog";
import type { RemoteProviderPreview } from "../lib/api";

const preview: RemoteProviderPreview = {
  id: "remote-kimi",
  name: "Remote Kimi",
  description: "A remote provider",
  runtime: "node",
  sourceUrl: "https://example.com/provider.cjs",
  requiredEnvVars: ["KIMI_API_KEY"],
  checksum: "sha256:abc123"
};

describe("InstallRemoteProviderDialog", () => {
  test("renders_nothing_when_preview_is_null", () => {
    const { container } = render(
      <InstallRemoteProviderDialog preview={null} loading={false} onConfirm={vi.fn()} onCancel={vi.fn()} />
    );
    expect(container.firstChild).toBeNull();
  });

  test("displays_provider_details_and_required_env_vars", () => {
    render(
      <InstallRemoteProviderDialog preview={preview} loading={false} onConfirm={vi.fn()} onCancel={vi.fn()} />
    );

    expect(screen.getByText("Install Remote Provider")).toBeInTheDocument();
    expect(screen.getByText("Remote Kimi")).toBeInTheDocument();
    expect(screen.getByText("remote-kimi")).toBeInTheDocument();
    expect(screen.getByText("node")).toBeInTheDocument();
    expect(screen.getByText("https://example.com/provider.cjs")).toBeInTheDocument();
    expect(screen.getByText("KIMI_API_KEY")).toBeInTheDocument();
    expect(screen.getByText("sha256:abc123")).toBeInTheDocument();
  });

  test("confirm_button_calls_onConfirm", () => {
    const onConfirm = vi.fn();
    render(
      <InstallRemoteProviderDialog preview={preview} loading={false} onConfirm={onConfirm} onCancel={vi.fn()} />
    );

    fireEvent.click(screen.getByRole("button", { name: "Install" }));
    expect(onConfirm).toHaveBeenCalledTimes(1);
  });

  test("cancel_button_calls_onCancel", () => {
    const onCancel = vi.fn();
    render(
      <InstallRemoteProviderDialog preview={preview} loading={false} onConfirm={vi.fn()} onCancel={onCancel} />
    );

    fireEvent.click(screen.getByRole("button", { name: "Cancel" }));
    expect(onCancel).toHaveBeenCalledTimes(1);
  });

  test("disables_buttons_when_loading", () => {
    render(
      <InstallRemoteProviderDialog preview={preview} loading={true} onConfirm={vi.fn()} onCancel={vi.fn()} />
    );

    expect(screen.getByRole("button", { name: "Installing..." })).toBeDisabled();
    expect(screen.getByRole("button", { name: "Cancel" })).toBeDisabled();
  });
});
