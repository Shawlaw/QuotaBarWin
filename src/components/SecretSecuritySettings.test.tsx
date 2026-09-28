import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, test, vi } from "vitest";
import { SecretSecuritySettings } from "./SecretSecuritySettings";
import { I18nProvider } from "../i18n";
import {
  disableManagedSecretsEncryption,
  enableManagedSecretsEncryption,
  getManagedSecretsEncryptionStatus,
} from "../lib/api";

vi.mock("../lib/api", () => ({
  getManagedSecretsEncryptionStatus: vi.fn(),
  enableManagedSecretsEncryption: vi.fn(),
  disableManagedSecretsEncryption: vi.fn(),
}));

const statusMock = vi.mocked(getManagedSecretsEncryptionStatus);
const enableMock = vi.mocked(enableManagedSecretsEncryption);
const disableMock = vi.mocked(disableManagedSecretsEncryption);

function renderSettings() {
  return render(
    <I18nProvider language="en">
      <SecretSecuritySettings />
    </I18nProvider>,
  );
}

afterEach(() => {
  statusMock.mockReset();
  enableMock.mockReset();
  disableMock.mockReset();
});

describe("SecretSecuritySettings", () => {
  test("shows the encrypted state and offers to disable", async () => {
    statusMock.mockResolvedValue({
      storage: "encrypted",
      promptPending: false,
      plaintextCount: 0,
      encryptedCount: 2,
    });

    renderSettings();

    expect(await screen.findByTestId("secret-security-state")).toHaveTextContent(
      /Encrypted storage enabled/,
    );
    expect(screen.getByTestId("secret-security-detail")).toHaveTextContent("2");
    expect(screen.getByTestId("secret-security-disable")).toBeInTheDocument();
  });

  test("enabling encryption migrates and refreshes the status", async () => {
    statusMock.mockResolvedValueOnce({
      storage: "plaintext",
      promptPending: false,
      plaintextCount: 1,
      encryptedCount: 0,
    });
    statusMock.mockResolvedValueOnce({
      storage: "encrypted",
      promptPending: false,
      plaintextCount: 0,
      encryptedCount: 1,
    });
    enableMock.mockResolvedValue(1);

    renderSettings();

    const enable = await screen.findByTestId("secret-security-enable");
    fireEvent.click(enable);

    await waitFor(() => expect(enableMock).toHaveBeenCalledTimes(1));
    expect(await screen.findByTestId("secret-security-state")).toHaveTextContent(
      /Encrypted storage enabled/,
    );
  });

  test("surfaces failures without switching the displayed state", async () => {
    statusMock.mockResolvedValue({
      storage: "plaintext",
      promptPending: false,
      plaintextCount: 1,
      encryptedCount: 0,
    });
    enableMock.mockRejectedValue(new Error("dpapi failed"));

    renderSettings();

    fireEvent.click(await screen.findByTestId("secret-security-enable"));

    expect(await screen.findByRole("alert")).toHaveTextContent(/failed/i);
    expect(screen.getByTestId("secret-security-state")).toHaveTextContent(
      /Encryption disabled/,
    );
  });
});
