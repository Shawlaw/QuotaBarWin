import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";
import { I18nProvider } from "../../i18n";
import { ProviderSetupPage } from "./ProviderSetupPage";
import type { ProviderSetupDescriptor } from "../../types";

const apiMocks = vi.hoisted(() => ({
  getProviderSetup: vi.fn(),
  saveProviderSetup: vi.fn(),
  testProviderSetup: vi.fn(),
}));

vi.mock("../../lib/api", () => apiMocks);

const descriptor: ProviderSetupDescriptor = {
  providerId: "kimi-work",
  providerType: "kimi-coding",
  displayName: "Kimi work",
  setupState: "pending",
  hasUnknownEnvVars: false,
  canAutoDetect: false,
  fields: [
    {
      name: "API_KEY",
      label: "API key",
      kind: "secret",
      required: true,
      configured: true,
      source: "managedLocalFile",
    },
    {
      name: "REGION",
      label: "Region",
      kind: "string",
      required: true,
      value: "CN",
      configured: true,
      source: "literal",
    },
    {
      name: "RETRIES",
      label: "Retries",
      kind: "number",
      required: false,
      value: 3,
      configured: true,
      source: "literal",
      advanced: true,
    },
    {
      name: "MODE",
      label: "Mode",
      kind: "select",
      required: false,
      options: ["fast", "safe"],
      value: "safe",
      configured: true,
      source: "literal",
      advanced: true,
    },
  ],
};

afterEach(() => {
  vi.clearAllMocks();
});

function renderPage(
  setupDescriptor: ProviderSetupDescriptor = descriptor,
  navigation: {
    closeRequest?: number;
    onRequestClose?: () => void;
  } = {},
) {
  const onConfigChanged = vi.fn(async () => undefined);
  const onComplete = vi.fn();
  const onBack = vi.fn();
  const onRequestClose = navigation.onRequestClose ?? vi.fn();
  apiMocks.getProviderSetup.mockResolvedValue(setupDescriptor);
  apiMocks.saveProviderSetup.mockResolvedValue({ ...setupDescriptor, setupState: "unverified" });
  apiMocks.testProviderSetup.mockResolvedValue({
    success: true,
    provider: { windows: [{ id: "daily" }] },
  });
  render(
    <I18nProvider language="en">
      <ProviderSetupPage
        providerId={setupDescriptor.providerId}
        onBack={onBack}
        onComplete={onComplete}
        onConfigChanged={onConfigChanged}
        closeRequest={navigation.closeRequest ?? 0}
        onRequestClose={onRequestClose}
      />
    </I18nProvider>,
  );
  return { onBack, onComplete, onConfigChanged, onRequestClose };
}

test("provider setup renders structured fields without echoing configured secret", async () => {
  renderPage();

  expect(await screen.findByTestId("provider-setup-page")).toBeInTheDocument();
  expect(screen.getByLabelText("API key")).toHaveValue("");
  expect(screen.getByText("Configured")).toBeInTheDocument();
  expect(screen.getByText("After you save, QuotaBarWin creates an isolated local file for this Provider instance at <config folder>\\secrets\\providers\\kimi-work\\API_KEY.txt. The main configuration keeps only a reference, and the value is never shown here.")).toBeInTheDocument();
  expect(screen.getByText("Leave the field empty to keep the current credential. Enter a new value only when you want to replace it.")).toBeInTheDocument();
  expect(screen.getByLabelText("Region")).toHaveValue("CN");
  expect(screen.queryByLabelText("Retries")).not.toBeInTheDocument();

  fireEvent.click(screen.getByRole("button", { name: "Advanced settings" }));
  expect(screen.getByLabelText("Retries")).toHaveValue(3);
  expect(screen.getByLabelText("Mode")).toHaveValue("safe");
});

test("provider setup saves secret only in the request and tests through the backend", async () => {
  const { onConfigChanged } = renderPage();
  await screen.findByTestId("provider-setup-page");

  fireEvent.change(screen.getByLabelText("API key"), { target: { value: "entered-value" } });
  fireEvent.change(screen.getByLabelText("Region"), { target: { value: "US" } });
  fireEvent.click(screen.getByRole("button", { name: "Save and test" }));

  await waitFor(() => expect(apiMocks.saveProviderSetup).toHaveBeenCalledWith({
    providerId: "kimi-work",
    displayName: "Kimi work",
    values: { REGION: "US", RETRIES: 3, MODE: "safe" },
    secretUpdates: { API_KEY: "entered-value" },
  }));
  await waitFor(() => expect(apiMocks.testProviderSetup).toHaveBeenCalledWith("kimi-work"));
  expect(onConfigChanged).toHaveBeenCalledTimes(2);
  expect(onConfigChanged).toHaveBeenLastCalledWith({
    success: true,
    provider: { windows: [{ id: "daily" }] },
  });
  expect(await screen.findByText("Configuration works. This Provider is now enabled.")).toBeInTheDocument();
  expect(screen.getByLabelText("API key")).toHaveValue("");
});

test("provider setup keeps the page open after a failed test", async () => {
  renderPage();
  apiMocks.testProviderSetup.mockResolvedValueOnce({ success: false, provider: null });
  await screen.findByTestId("provider-setup-page");

  fireEvent.click(screen.getByRole("button", { name: "Save and test" }));

  expect(await screen.findByText("Your configuration was saved safely but is still disabled. Update it and try again.")).toBeInTheDocument();
  expect(screen.getByTestId("provider-setup-page")).toBeInTheDocument();
});

test("provider setup does not replay an earlier close request on mount", async () => {
  const onRequestClose = vi.fn();
  renderPage(descriptor, { closeRequest: 1, onRequestClose });

  expect(await screen.findByTestId("provider-setup-page")).toBeInTheDocument();
  await waitFor(() => expect(apiMocks.getProviderSetup).toHaveBeenCalledWith("kimi-work"));
  expect(onRequestClose).not.toHaveBeenCalled();
});

test("providers with only optional fields still save and test through the shared flow", async () => {
  renderPage({
    ...descriptor,
    providerId: "codex-local",
    providerType: "codex-usage",
    canAutoDetect: true,
    fields: [
      {
        name: "CODEX_ACCOUNT_ID",
        label: "ChatGPT account id",
        kind: "string",
        required: false,
        configured: false,
        source: "default",
      },
    ],
  });
  await screen.findByTestId("provider-setup-page");

  expect(screen.queryByText("This Provider first tries to use its local sign-in information. You can test it without filling the optional fields.")).not.toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: "Save and test" }));

  await waitFor(() => expect(apiMocks.saveProviderSetup).toHaveBeenCalledWith({
    providerId: "codex-local",
    displayName: "Kimi work",
    values: { CODEX_ACCOUNT_ID: null },
    secretUpdates: {},
  }));
  await waitFor(() => expect(apiMocks.testProviderSetup).toHaveBeenCalledWith("codex-local"));
});
