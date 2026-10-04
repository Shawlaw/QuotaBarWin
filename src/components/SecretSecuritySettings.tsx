import { useCallback, useEffect, useState } from "react";
import { useI18n } from "../i18n";
import {
  disableManagedSecretsEncryption,
  enableManagedSecretsEncryption,
  getManagedSecretsEncryptionStatus,
} from "../lib/api";
import type { ManagedSecretsEncryptionStatus } from "../types";

// Settings section that shows the managed secret storage mode and lets the
// user switch between DPAPI-encrypted and plaintext storage at any time.
export function SecretSecuritySettings() {
  const { t } = useI18n();
  const [status, setStatus] = useState<ManagedSecretsEncryptionStatus | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState(false);

  useEffect(() => {
    let mounted = true;
    getManagedSecretsEncryptionStatus()
      .then((loaded) => {
        if (mounted) {
          setStatus(loaded);
        }
      })
      .catch(() => undefined);
    return () => {
      mounted = false;
    };
  }, []);

  const runAction = useCallback(async (action: () => Promise<number>) => {
    setBusy(true);
    setError(false);
    try {
      await action();
      setStatus(await getManagedSecretsEncryptionStatus());
    } catch {
      setError(true);
    } finally {
      setBusy(false);
    }
  }, []);

  if (!status) {
    return null;
  }

  const encrypted = status.storage === "encrypted";

  return (
    <section
      aria-label={t.secretSecurity.settingsTitle}
      className="settings-section secret-security-section"
      data-testid="secret-security-section"
    >
      <div className="settings-section-title">
        <h3>{t.secretSecurity.settingsTitle}</h3>
        <span data-testid="secret-security-state">
          {encrypted ? t.secretSecurity.statusEncrypted : t.secretSecurity.statusPlaintext}
        </span>
      </div>
      <div className="secret-security-section__body">
        <p data-testid="secret-security-detail">
          {t.secretSecurity.statusDetail(status.plaintextCount, status.encryptedCount)}
        </p>
        <p className="secret-security-section__note" data-testid="secret-security-note">
          {t.secretSecurity.settingsNote}
        </p>
        {error ? (
          <p role="alert">{t.secretSecurity.failed}</p>
        ) : null}
        <button
          className="button-secondary"
          data-testid={
            encrypted ? "secret-security-disable" : "secret-security-enable"
          }
          disabled={busy}
          onClick={() =>
            void runAction(
              encrypted ? disableManagedSecretsEncryption : enableManagedSecretsEncryption
            )
          }
          type="button"
        >
          {busy
            ? t.secretSecurity.working
            : encrypted
              ? t.secretSecurity.disableAction
              : t.secretSecurity.enableAction}
        </button>
      </div>
    </section>
  );
}
