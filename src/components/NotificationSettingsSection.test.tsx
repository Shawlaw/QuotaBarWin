import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { vi } from "vitest";
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
  render(
    <I18nProvider language="en">
      <NotificationSettingsSection settings={settings} onChange={onChange} />
    </I18nProvider>,
  );
  return { onChange };
}

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

    fireEvent.click(screen.getByTestId("notification-event-quota-reset"));
    expect(onChange).toHaveBeenLastCalledWith({
      ...baseSettings,
      events: ["quota-exhausted", "provider-error"],
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
      expect(apiMocks.sendTestNotification).toHaveBeenCalled();
    });
    await waitFor(() => {
      expect(screen.getByTestId("notification-test-message")).toHaveTextContent(/HTTP 200/);
    });
  });
});
