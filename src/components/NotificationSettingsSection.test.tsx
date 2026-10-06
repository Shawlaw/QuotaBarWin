import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { useState } from "react";
import { beforeEach, vi } from "vitest";
import { NotificationSettingsSection } from "./NotificationSettingsSection";
import { I18nProvider } from "../i18n";
import type { NotificationSettings } from "../types";

const apiMocks = vi.hoisted(() => ({
  sendTestNotification: vi.fn(async () => ({
    toast: { status: "sent", detail: null },
    webhook: { status: "sent", detail: null, statusCode: 200 },
  })),
}));

vi.mock("../lib/api", () => apiMocks);

const baseSettings: NotificationSettings = {
  toastEnabled: false,
  webhookEnabled: false,
  webhookUrl: null,
  webhookTimeoutSeconds: 10,
  events: ["quota-reset", "quota-exhausted", "provider-error"],
};

function renderSection(settings: NotificationSettings = baseSettings) {
  const onChange = vi.fn();
  const onLowQuotaWarningThresholdChange = vi.fn();
  // Mirrors the real settings page: edits flow through the parent so the
  // section always sees the live unsaved draft.
  function Harness() {
    const [draft, setDraft] = useState(settings);
    return (
      <I18nProvider language="en">
        <NotificationSettingsSection
          settings={draft}
          onChange={(next) => {
            onChange(next);
            setDraft(next);
          }}
          lowQuotaWarningThreshold={20}
          onLowQuotaWarningThresholdChange={onLowQuotaWarningThresholdChange}
        />
      </I18nProvider>
    );
  }
  render(<Harness />);
  return { onChange, onLowQuotaWarningThresholdChange };
}

beforeEach(() => {
  vi.clearAllMocks();
});

describe("NotificationSettingsSection", () => {
  test("toggles propagate updated notification settings", () => {
    const { onChange } = renderSection();

    fireEvent.click(screen.getByTestId("notification-toast-enabled"));
    expect(onChange).toHaveBeenCalledWith({
      ...baseSettings,
      toastEnabled: true,
    });
  });

  test("event type checkboxes add and remove selected events", () => {
    const { onChange } = renderSection();

    fireEvent.click(screen.getByTestId("notification-event-quota-low"));
    expect(onChange).toHaveBeenLastCalledWith({
      ...baseSettings,
      // Selections are re-serialized in the canonical event-type order.
      events: ["quota-reset", "quota-exhausted", "quota-low", "provider-error"],
    });

    // The draft accumulates like the real settings page: removing quota-reset
    // keeps the quota-low selection added above.
    fireEvent.click(screen.getByTestId("notification-event-quota-reset"));
    expect(onChange).toHaveBeenLastCalledWith({
      ...baseSettings,
      events: ["quota-exhausted", "quota-low", "provider-error"],
    });
  });

  test("webhook url and timeout inputs only appear when webhook is enabled", () => {
    const { onChange } = renderSection();
    expect(screen.queryByTestId("notification-webhook-url")).not.toBeInTheDocument();

    fireEvent.click(screen.getByTestId("notification-webhook-enabled"));
    expect(onChange).toHaveBeenLastCalledWith({ ...baseSettings, webhookEnabled: true });
  });

  test("invalid timeout values are rejected with an error message", () => {
    renderSection({ ...baseSettings, webhookEnabled: true, webhookUrl: "https://example.com/hook" });

    const timeoutInput = screen.getByTestId("notification-webhook-timeout");
    fireEvent.change(timeoutInput, { target: { value: "500" } });
    expect(screen.getByText(/between 1 and 60/i)).toBeInTheDocument();
  });

  test("the test button reports both channel outcomes", async () => {
    renderSection();

    fireEvent.click(screen.getByTestId("notification-test-button"));

    await waitFor(() => {
      expect(apiMocks.sendTestNotification).toHaveBeenCalledWith(baseSettings);
    });
    await waitFor(() => {
      expect(screen.getByTestId("notification-test-message")).toHaveTextContent(/HTTP 200/);
    });
  });

  test("the test button uses the unsaved form draft", async () => {
    renderSection({ ...baseSettings, webhookEnabled: true, webhookUrl: "https://example.com/old-hook" });

    fireEvent.change(screen.getByTestId("notification-webhook-url"), {
      target: { value: "https://example.com/new-hook" },
    });
    fireEvent.click(screen.getByTestId("notification-test-button"));

    await waitFor(() => {
      expect(apiMocks.sendTestNotification).toHaveBeenCalledWith({
        ...baseSettings,
        webhookEnabled: true,
        webhookUrl: "https://example.com/new-hook",
      });
    });
  });

  test("events render grouped by quota / provider / app categories", () => {
    renderSection();

    const quotaGroup = screen.getByTestId("notification-event-grid-quota");
    expect(quotaGroup).toHaveTextContent("Quota reset");
    expect(quotaGroup).toHaveTextContent("Unexpected quota recovery");
    expect(quotaGroup).toHaveTextContent("Quota exhausted");
    expect(quotaGroup).toHaveTextContent("Quota low");
    expect(screen.getByText("Quota")).toBeInTheDocument();

    const providerGroup = screen.getByTestId("notification-event-grid-provider");
    expect(providerGroup).toHaveTextContent("Provider failure");
    expect(providerGroup).toHaveTextContent("Provider recovered");

    const appGroup = screen.getByTestId("notification-event-grid-app");
    expect(appGroup).toHaveTextContent("Application updated");
    expect(appGroup).toHaveTextContent("Application started");
  });

  test("the low quota threshold input is the single editing entry", () => {
    const { onLowQuotaWarningThresholdChange } = renderSection();

    fireEvent.change(screen.getByTestId("low-quota-warning-input"), { target: { value: "35" } });
    expect(onLowQuotaWarningThresholdChange).toHaveBeenCalledWith(35);
  });
});
