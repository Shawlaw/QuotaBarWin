import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, test, vi } from "vitest";
import { SecretEncryptionPrompt } from "./SecretEncryptionPrompt";
import { I18nProvider } from "../i18n";
import type { ManagedSecretsEncryptionStatus } from "../types";

function renderPrompt(
  status: ManagedSecretsEncryptionStatus | null,
  props: Partial<Parameters<typeof SecretEncryptionPrompt>[0]> = {},
) {
  return render(
    <I18nProvider language="en">
      <SecretEncryptionPrompt
        status={status}
        busy={false}
        error={false}
        onEnable={vi.fn()}
        onDismiss={vi.fn()}
        {...props}
      />
    </I18nProvider>,
  );
}

const pendingWithPlaintext: ManagedSecretsEncryptionStatus = {
  storage: "plaintext",
  promptPending: true,
  plaintextCount: 3,
  encryptedCount: 0,
};

describe("SecretEncryptionPrompt", () => {
  test("renders nothing without a pending prompt", () => {
    renderPrompt(null);
    renderPrompt({ ...pendingWithPlaintext, promptPending: false });
    expect(screen.queryByTestId("secret-encryption-prompt")).not.toBeInTheDocument();
  });

  test("mentions the plaintext secret count when secrets exist", () => {
    renderPrompt(pendingWithPlaintext);
    expect(screen.getByTestId("secret-encryption-prompt-body")).toHaveTextContent("3");
  });

  test("falls back to the enable-only copy without existing secrets", () => {
    renderPrompt({ ...pendingWithPlaintext, plaintextCount: 0 });
    expect(screen.getByTestId("secret-encryption-prompt-body")).toHaveTextContent(
      /No saved secrets yet/,
    );
  });

  test("forwards confirmations and deferrals", () => {
    const onEnable = vi.fn();
    const onDismiss = vi.fn();
    renderPrompt(pendingWithPlaintext, { onEnable, onDismiss });

    fireEvent.click(screen.getByTestId("secret-encryption-enable"));
    expect(onEnable).toHaveBeenCalledTimes(1);

    fireEvent.click(screen.getByTestId("secret-encryption-dismiss"));
    expect(onDismiss).toHaveBeenCalledTimes(1);
  });

  test("shows a failure message and keeps buttons usable", () => {
    renderPrompt(pendingWithPlaintext, { error: true });
    expect(screen.getByRole("alert")).toHaveTextContent(/failed/i);
    expect(screen.getByTestId("secret-encryption-enable")).toBeEnabled();
  });
});
