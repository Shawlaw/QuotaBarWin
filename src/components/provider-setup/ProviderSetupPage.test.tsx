import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";
import { I18nProvider } from "../../i18n";
import { ProviderSetupPage } from "./ProviderSetupPage";
import type { ProviderSetupDescriptor } from "../../types";

const apiMocks = vi.hoisted(() => ({
  adoptParameterAsManagedSecret: vi.fn(),
  copyManagedSecret: vi.fn(),
  getConfig: vi.fn(async () => ({ providers: [] }) as never),
  getProviderSetup: vi.fn(),
  openExternalLink: vi.fn(async () => undefined),
  revealManagedSecret: vi.fn(),
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

test("provider setup opens field guide links through the external browser", async () => {
  const openExternalLinkMock = vi.mocked(apiMocks.openExternalLink);
  const withGuide: ProviderSetupDescriptor = {
    ...descriptor,
    fields: descriptor.fields.map((field) =>
      field.name === "API_KEY"
        ? { ...field, helpUrl: "https://platform.example.com/api-keys" }
        : field,
    ),
  };

  renderPage(withGuide);
  const guide = await screen.findByRole("link", { name: "View guide" });
  fireEvent.click(guide);

  expect(openExternalLinkMock).toHaveBeenCalledWith("https://platform.example.com/api-keys");
});

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

test("provider setup can view and copy a managed credential without saving it", async () => {
  apiMocks.revealManagedSecret.mockResolvedValue({ value: "sk-managed-secret" });
  apiMocks.copyManagedSecret.mockResolvedValue(undefined);

  renderPage();

  const revealButton = await screen.findByTestId("reveal-secret-API_KEY");
  vi.useFakeTimers();
  try {
    await act(async () => {
      fireEvent.click(revealButton);
    });
    expect(screen.getByTestId("revealed-secret-API_KEY")).toHaveTextContent(
      "sk-managed-secret"
    );
    expect(apiMocks.revealManagedSecret).toHaveBeenCalledWith("kimi-work", "API_KEY");
    await act(async () => {
      await vi.advanceTimersByTimeAsync(16000);
    });
    expect(screen.getByTestId("revealed-secret-API_KEY")).toHaveTextContent(
      "sk-managed-secret"
    );
  } finally {
    vi.useRealTimers();
  }

  fireEvent.click(screen.getByTestId("copy-secret-API_KEY"));
  await waitFor(() =>
    expect(apiMocks.copyManagedSecret).toHaveBeenCalledWith("kimi-work", "API_KEY")
  );
  expect(
    await screen.findByText("Copied.")
  ).toBeInTheDocument();
  expect(screen.getByTestId("revealed-secret-API_KEY")).toHaveTextContent("sk-managed-secret");

  fireEvent.click(screen.getByRole("button", { name: "Hide" }));
  expect(screen.queryByTestId("revealed-secret-API_KEY")).not.toBeInTheDocument();
  expect(screen.getByTestId("copy-secret-API_KEY")).toBeEnabled();
});

test("provider setup collapses optional credentials and lists undeclared variables", async () => {
  const grouped: ProviderSetupDescriptor = {
    ...descriptor,
    unknownEnvVars: ["LEGACY_FLAG"],
    fields: [
      ...descriptor.fields,
      {
        name: "OPTIONAL_TEAM",
        label: "Team",
        kind: "secret",
        required: false,
        configured: false,
        source: "missing",
      },
    ],
  };

  renderPage(grouped);

  expect(await screen.findByText("Optional credentials (1)")).toBeInTheDocument();
  expect(screen.getByTestId("provider-setup-unknown-vars")).toHaveTextContent("LEGACY_FLAG");
});

test("providers with no fields can save and test", async () => {
  renderPage({ ...descriptor, fields: [], canAutoDetect: true });
  fireEvent.click(await screen.findByRole("button", { name: "Save and test" }));
  await waitFor(() => expect(apiMocks.saveProviderSetup).toHaveBeenCalledWith({
    providerId: "kimi-work",
    displayName: "Kimi work",
    values: {},
    secretUpdates: {},
  }));
  await waitFor(() => expect(apiMocks.testProviderSetup).toHaveBeenCalledWith("kimi-work"));
});

test("multiple credentials keep blank replacements and send only explicit changes", async () => {
  const field = descriptor.fields[0];
  renderPage({
    ...descriptor,
    fields: [field, { ...field, name: "SECOND_KEY", label: "Second key", required: false }],
  });
  const first = await screen.findByLabelText("API key");
  const second = screen.getByLabelText("Second key");
  fireEvent.change(first, { target: { value: "temporary" } });
  fireEvent.change(first, { target: { value: "" } });
  fireEvent.change(second, { target: { value: "temporary" } });
  fireEvent.change(second, { target: { value: "   " } });
  fireEvent.click(screen.getByRole("button", { name: "Save without testing" }));
  await waitFor(() => expect(apiMocks.saveProviderSetup).toHaveBeenCalledWith({
    providerId: "kimi-work", displayName: "Kimi work", values: {}, secretUpdates: {},
  }));
  await waitFor(() => expect(screen.getByRole("button", { name: "Save without testing" })).toBeEnabled());

  apiMocks.revealManagedSecret.mockResolvedValueOnce({ value: "first-key" }).mockResolvedValueOnce({ value: "second-key" });
  fireEvent.click(screen.getByTestId("reveal-secret-API_KEY"));
  expect(await screen.findByTestId("revealed-secret-API_KEY")).toHaveTextContent("first-key");
  fireEvent.click(screen.getByTestId("reveal-secret-SECOND_KEY"));
  expect(await screen.findByTestId("revealed-secret-SECOND_KEY")).toHaveTextContent("second-key");
  expect(screen.getByTestId("revealed-secret-API_KEY")).toHaveTextContent("first-key");
  fireEvent.click(screen.getByTestId("copy-secret-SECOND_KEY"));
  await waitFor(() => expect(apiMocks.copyManagedSecret).toHaveBeenCalledWith("kimi-work", "SECOND_KEY"));
  await waitFor(() => expect(screen.getByRole("button", { name: "Save without testing" })).toBeEnabled());

  fireEvent.change(screen.getByLabelText("API key"), { target: { value: "replacement" } });
  fireEvent.click(screen.getAllByRole("button", { name: "Clear" })[1]);
  fireEvent.click(screen.getByRole("button", { name: "Save without testing" }));
  await waitFor(() => expect(apiMocks.saveProviderSetup).toHaveBeenLastCalledWith({
    providerId: "kimi-work", displayName: "Kimi work", values: {},
    secretUpdates: { API_KEY: "replacement", SECOND_KEY: null },
  }));
});

test("provider setup can move a plaintext credential to the managed store", async () => {
  const literalDescriptor: ProviderSetupDescriptor = {
    ...descriptor,
    fields: descriptor.fields.map((field) =>
      field.name === "API_KEY" ? { ...field, source: "literal" } : field
    ),
  };
  const managedDescriptor: ProviderSetupDescriptor = {
    ...literalDescriptor,
    fields: literalDescriptor.fields.map((field) =>
      field.name === "API_KEY" ? { ...field, source: "managedLocalFile" } : field
    ),
  };
  apiMocks.adoptParameterAsManagedSecret.mockResolvedValue({
    descriptor: managedDescriptor,
    adopted: true,
  });

  const { onConfigChanged } = renderPage(literalDescriptor);
  const adopt = await screen.findByTestId("adopt-secret-API_KEY");

  fireEvent.click(adopt);

  await waitFor(() =>
    expect(apiMocks.adoptParameterAsManagedSecret).toHaveBeenCalledWith(
      "kimi-work",
      "API_KEY"
    )
  );
  await waitFor(() => expect(onConfigChanged).toHaveBeenCalled());
  await waitFor(() => expect(screen.queryByTestId("adopt-secret-API_KEY")).toBeNull());
});
