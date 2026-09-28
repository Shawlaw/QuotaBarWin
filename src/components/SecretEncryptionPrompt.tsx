import { useI18n } from "../i18n";
import type { ManagedSecretsEncryptionStatus } from "../types";

type SecretEncryptionPromptProps = {
  status: ManagedSecretsEncryptionStatus | null;
  busy: boolean;
  error: boolean;
  onEnable: () => void;
  onDismiss: () => void;
};

// One-time startup dialog that asks upgrading users whether to move managed
// Provider secrets into DPAPI-encrypted storage.
export function SecretEncryptionPrompt({
  status,
  busy,
  error,
  onEnable,
  onDismiss,
}: SecretEncryptionPromptProps) {
  const { t } = useI18n();
  if (!status || !status.promptPending) {
    return null;
  }

  return (
    <div
      className="secret-encryption-backdrop"
      data-testid="secret-encryption-prompt-backdrop"
    >
      <div
        aria-labelledby="secret-encryption-prompt-title"
        aria-modal="true"
        className="secret-encryption-prompt"
        data-testid="secret-encryption-prompt"
        role="dialog"
      >
        <h2 id="secret-encryption-prompt-title">{t.secretSecurity.promptTitle}</h2>
        <p>{t.secretSecurity.promptIntro}</p>
        <p data-testid="secret-encryption-prompt-body">
          {status.plaintextCount > 0
            ? t.secretSecurity.promptMigrateBody(status.plaintextCount)
            : t.secretSecurity.promptEnableBody}
        </p>
        <p className="secret-encryption-prompt__note">
          {t.secretSecurity.promptPortabilityNote}
        </p>
        {error ? (
          <p className="secret-encryption-prompt__error" role="alert">
            {t.secretSecurity.failed}
          </p>
        ) : null}
        <div className="secret-encryption-prompt__actions">
          <button
            data-testid="secret-encryption-enable"
            disabled={busy}
            onClick={onEnable}
            type="button"
          >
            {busy ? t.secretSecurity.migrating : t.secretSecurity.promptEnable}
          </button>
          <button
            className="button-link"
            data-testid="secret-encryption-dismiss"
            disabled={busy}
            onClick={onDismiss}
            type="button"
          >
            {t.secretSecurity.promptLater}
          </button>
        </div>
      </div>
    </div>
  );
}
