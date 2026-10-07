import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { useState } from "react";
import { beforeEach, vi } from "vitest";
import { NotificationSettingsSection } from "./NotificationSettingsSection";
import { I18nProvider } from "../i18n";
import type { NotificationSettings, WebhookEndpoint } from "../types";

const apiMocks = vi.hoisted(() => ({
  sendTestNotification: vi.fn(async () => ({
    toast: { status: "sent", detail: null },
    webhooks: [
      { id: "webhook-1", label: "#1", status: "sent", detail: null, statusCode: 200 },
      { id: "webhook-2", label: "DingTalk", status: "failed", detail: "boom", statusCode: null },
    ],
  })),
  openWebhookTemplateGuide: vi.fn(async () => {}),
}));

vi.mock("../lib/api", () => apiMocks);

function endpoint(overrides: Partial<WebhookEndpoint> = {}): WebhookEndpoint {
  return {
    id: "webhook-1",
    name: null,
    url: "https://example.com/hook",
    timeoutSeconds: 10,
    template: null,
    enabled: true,
    ...overrides,
  };
}

const baseSettings: NotificationSettings = {
  toastEnabled: false,
  webhookEnabled: false,
  webhooks: [],
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

  test("webhook endpoint controls only appear when webhook is enabled", () => {
    const { onChange } = renderSection();
    expect(screen.queryByTestId("notification-webhook-url-0")).not.toBeInTheDocument();
    expect(screen.queryByTestId("notification-webhook-add")).not.toBeInTheDocument();

    fireEvent.click(screen.getByTestId("notification-webhook-enabled"));
    expect(onChange).toHaveBeenLastCalledWith({ ...baseSettings, webhookEnabled: true });
  });

  test("adding and removing webhook endpoints updates the draft", () => {
    const { onChange } = renderSection({
      ...baseSettings,
      webhookEnabled: true,
      webhooks: [endpoint()],
    });

    fireEvent.click(screen.getByTestId("notification-webhook-remove-0"));
    expect(onChange).toHaveBeenLastCalledWith(
      expect.objectContaining({ webhooks: [] }),
    );

    fireEvent.click(screen.getByTestId("notification-webhook-add"));
    const added = (onChange.mock.lastCall?.[0] as NotificationSettings).webhooks;
    expect(added).toHaveLength(1);
    expect(added[0].url).toBe("");
    expect(added[0].enabled).toBe(true);
    expect(added[0].timeoutSeconds).toBe(10);
  });

  test("endpoint field edits update the matching endpoint only", () => {
    const { onChange } = renderSection({
      ...baseSettings,
      webhookEnabled: true,
      webhooks: [
        endpoint(),
        endpoint({ id: "webhook-2", url: "https://example.com/other" }),
      ],
    });

    fireEvent.change(screen.getByTestId("notification-webhook-url-0"), {
      target: { value: "https://example.com/first " },
    });
    expect(onChange).toHaveBeenLastCalledWith(
      expect.objectContaining({
        webhooks: [
          expect.objectContaining({ id: "webhook-1", url: "https://example.com/first" }),
          expect.objectContaining({ id: "webhook-2", url: "https://example.com/other" }),
        ],
      }),
    );

    fireEvent.change(screen.getByTestId("notification-webhook-template-1"), {
      target: { value: ' {"text":"{{message}}"} ' },
    });
    expect(onChange).toHaveBeenLastCalledWith(
      expect.objectContaining({
        webhooks: [
          expect.objectContaining({ id: "webhook-1", template: null }),
          expect.objectContaining({ id: "webhook-2", template: '{"text":"{{message}}"}' }),
        ],
      }),
    );

    fireEvent.change(screen.getByTestId("notification-webhook-name-1"), {
      target: { value: "DingTalk" },
    });
    expect(onChange).toHaveBeenLastCalledWith(
      expect.objectContaining({
        webhooks: [
          expect.objectContaining({ id: "webhook-1", name: null }),
          expect.objectContaining({ id: "webhook-2", name: "DingTalk" }),
        ],
      }),
    );

    fireEvent.click(screen.getByTestId("notification-webhook-endpoint-enabled-0"));
    expect(onChange).toHaveBeenLastCalledWith(
      expect.objectContaining({
        webhooks: [
          expect.objectContaining({ id: "webhook-1", enabled: false }),
          expect.objectContaining({ id: "webhook-2", enabled: true }),
        ],
      }),
    );
  });

  test("invalid timeout values are rejected with an error message", () => {
    renderSection({
      ...baseSettings,
      webhookEnabled: true,
      webhooks: [endpoint()],
    });

    fireEvent.change(screen.getByTestId("notification-webhook-timeout-0"), {
      target: { value: "500" },
    });
    expect(screen.getByText(/between 1 and 60/i)).toBeInTheDocument();
  });

  test("the test button reports one line per webhook endpoint", async () => {
    renderSection({
      ...baseSettings,
      webhookEnabled: true,
      webhooks: [endpoint()],
    });

    fireEvent.click(screen.getByTestId("notification-test-button"));

    await waitFor(() => {
      expect(screen.getByTestId("notification-test-message")).toHaveTextContent(
        /#1: Webhook delivered \(HTTP 200\)/,
      );
    });
    expect(screen.getByTestId("notification-test-message")).toHaveTextContent(
      /DingTalk: Webhook failed: boom/,
    );
  });

  test("the test button uses the unsaved form draft", async () => {
    renderSection({
      ...baseSettings,
      webhookEnabled: true,
      webhooks: [endpoint({ url: "https://example.com/old-hook" })],
    });

    fireEvent.change(screen.getByTestId("notification-webhook-url-0"), {
      target: { value: "https://example.com/new-hook" },
    });
    fireEvent.click(screen.getByTestId("notification-test-button"));

    await waitFor(() => {
      expect(apiMocks.sendTestNotification).toHaveBeenCalledWith(
        expect.objectContaining({
          webhooks: [expect.objectContaining({ url: "https://example.com/new-hook" })],
        }),
      );
    });
  });

  test("events render grouped by quota / provider / app categories", () => {
    renderSection();

    const quotaGroup = screen.getByTestId("notification-event-grid-quota");
    expect(quotaGroup).toHaveTextContent("Quota reset");
    expect(quotaGroup).toHaveTextContent("Quota expiry time changed");
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

  test("the webhook template guide button opens the built-in guide", () => {
    renderSection({
      ...baseSettings,
      webhookEnabled: true,
      webhooks: [endpoint()],
    });

    fireEvent.click(screen.getByTestId("notification-webhook-template-guide"));
    expect(apiMocks.openWebhookTemplateGuide).toHaveBeenCalledTimes(1);
  });
});
