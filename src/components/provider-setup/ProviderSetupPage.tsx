import { useEffect, useMemo, useRef, useState } from "react";
import { getProviderSetup, saveProviderSetup, testProviderSetup } from "../../lib/api";
import type {
  ProviderSetupDescriptor,
  ProviderSetupField,
  ProviderSetupTestResult,
} from "../../types";
import { useI18n } from "../../i18n";

type ProviderSetupPageProps = {
  providerId: string;
  onBack: () => void;
  onComplete: () => void;
  onConfigChanged: (testResult?: ProviderSetupTestResult) => Promise<void>;
  closeRequest: number;
  onRequestClose: () => void;
};

type FieldValues = Record<string, string>;
type SecretUpdates = Record<string, string | null>;

function fieldLabel(field: ProviderSetupField): string {
  return field.label?.trim() || field.name;
}

function initialFieldValues(descriptor: ProviderSetupDescriptor): FieldValues {
  return Object.fromEntries(
    descriptor.fields
      .filter((field) => field.kind !== "secret")
      .map((field) => [
        field.name,
        typeof field.value === "number" ? String(field.value) : String(field.value ?? ""),
      ]),
  );
}

function isAdvanced(field: ProviderSetupField): boolean {
  return field.advanced === true || !field.required;
}

export function ProviderSetupPage({
  providerId,
  onBack,
  onComplete,
  onConfigChanged,
  closeRequest,
  onRequestClose,
}: ProviderSetupPageProps) {
  const { t } = useI18n();
  const [descriptor, setDescriptor] = useState<ProviderSetupDescriptor | null>(null);
  const [displayName, setDisplayName] = useState("");
  const [values, setValues] = useState<FieldValues>({});
  const [secretUpdates, setSecretUpdates] = useState<SecretUpdates>({});
  const [showAdvanced, setShowAdvanced] = useState(false);
  const [message, setMessage] = useState<string | null>(null);
  const [result, setResult] = useState<ProviderSetupTestResult | null>(null);
  const [isBusy, setIsBusy] = useState(false);
  const [leaveTarget, setLeaveTarget] = useState<"back" | "close" | null>(null);
  // A close request is an event. The setup page can mount after a previous
  // settings navigation, so it must not treat that historic value as a new
  // request to leave the configuration flow.
  const handledCloseRequestRef = useRef(closeRequest);

  const visibleFields = useMemo(
    () =>
      descriptor?.fields.filter((field) => showAdvanced || !isAdvanced(field)) ?? [],
    [descriptor, showAdvanced],
  );
  const hasAdvancedFields = descriptor?.fields.some(isAdvanced) ?? false;
  const hasUnsavedChanges = useMemo(() => {
    if (!descriptor) {
      return false;
    }
    return displayName !== descriptor.displayName
      || JSON.stringify(values) !== JSON.stringify(initialFieldValues(descriptor))
      || Object.keys(secretUpdates).length > 0;
  }, [descriptor, displayName, secretUpdates, values]);

  async function loadSetup() {
    setMessage(null);
    const next = await getProviderSetup(providerId);
    setDescriptor(next);
    setDisplayName(next.displayName);
    setValues(initialFieldValues(next));
    setSecretUpdates({});
  }

  useEffect(() => {
    void loadSetup().catch((error) => {
      setMessage(error instanceof Error ? error.message : t.providerSetup.saveFailed);
    });
    // The setup page is keyed by providerId, so a provider change intentionally resets all sensitive draft state.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [providerId]);

  useEffect(() => {
    if (closeRequest === 0 || closeRequest <= handledCloseRequestRef.current) {
      return;
    }
    handledCloseRequestRef.current = closeRequest;
    if (hasUnsavedChanges) {
      setLeaveTarget("close");
    } else {
      onRequestClose();
    }
  }, [closeRequest, hasUnsavedChanges, onRequestClose]);

  function validate(): string | null {
    if (!descriptor || !displayName.trim()) {
      return t.providerSetup.missingRequired(t.providerSetup.accountName);
    }
    for (const field of descriptor.fields) {
      const label = fieldLabel(field);
      if (field.kind === "secret") {
        const update = secretUpdates[field.name];
        const configured = update === undefined ? field.configured : update !== null && update.trim().length > 0;
        if (field.required && !configured) {
          return t.providerSetup.missingRequired(label);
        }
        continue;
      }
      const value = values[field.name] ?? "";
      if (field.required && !value.trim()) {
        return t.providerSetup.missingRequired(label);
      }
      if (field.kind === "number" && value.trim() && !Number.isFinite(Number(value))) {
        return t.providerSetup.invalidNumber(label);
      }
    }
    return null;
  }

  async function save(runTest: boolean): Promise<boolean> {
    const validationError = validate();
    if (validationError || !descriptor) {
      setMessage(validationError);
      return false;
    }
    setIsBusy(true);
    setMessage(null);
    setResult(null);
    try {
      const serializedValues = Object.fromEntries(
        Object.entries(values).map(([name, value]) => {
          const field = descriptor.fields.find((candidate) => candidate.name === name);
          return [name, field?.kind === "number" && value.trim() ? Number(value) : value || null];
        }),
      );
      await saveProviderSetup({
        providerId,
        displayName: displayName.trim(),
        values: serializedValues,
        secretUpdates,
      });
      // The request is intentionally the only time this component holds a
      // secret. Immediately clear it before doing any follow-up work.
      setSecretUpdates({});
      await onConfigChanged();
      if (!runTest) {
        await loadSetup();
        return true;
      }
      const testResult = await testProviderSetup(providerId);
      setResult(testResult);
      await onConfigChanged(testResult);
      await loadSetup();
      setMessage(
        testResult.success
          ? t.providerSetup.testSucceeded
          : t.providerSetup.testFailedKeepSaved,
      );
      return true;
    } catch (error) {
      setMessage(error instanceof Error ? error.message : t.providerSetup.saveFailed);
      return false;
    } finally {
      setIsBusy(false);
    }
  }

  function requestLeave(target: "back" | "close") {
    if (hasUnsavedChanges) {
      setLeaveTarget(target);
      return;
    }
    if (target === "close") {
      onRequestClose();
    } else {
      onBack();
    }
  }

  async function saveAndLeave() {
    const target = leaveTarget;
    if (!target) {
      return;
    }
    if (await save(false)) {
      setLeaveTarget(null);
      if (target === "close") {
        onRequestClose();
      } else {
        onBack();
      }
    }
  }

  function updateSecret(field: ProviderSetupField, value: string) {
    setSecretUpdates((current) => ({ ...current, [field.name]: value }));
  }

  function clearSecret(field: ProviderSetupField) {
    setSecretUpdates((current) => ({ ...current, [field.name]: null }));
  }

  function renderField(field: ProviderSetupField) {
    const label = fieldLabel(field);
    const required = field.required ? <span className="provider-setup__required">{t.providerSetup.required}</span> : null;
    const sourceConfigured = secretUpdates[field.name] === undefined
      ? field.configured
      : secretUpdates[field.name] !== null && (secretUpdates[field.name]?.trim().length ?? 0) > 0;
    const credentialSourceHint = field.configured && field.source !== "managedLocalFile"
      ? t.providerSetup.credentialSource(t.providerSetup.credentialSourceType(field.source))
      : null;
    return (
      <label className="provider-setup__field" key={field.name}>
        <span>
          {label} {required}
        </span>
        {field.kind === "secret" ? (
          <>
            <div className="provider-setup__secret-status">
              <span>{sourceConfigured ? t.providerSetup.configured : t.providerSetup.notConfigured}</span>
              {field.configured ? (
                <button type="button" className="button-ghost" onClick={() => clearSecret(field)}>
                  {t.providerSetup.clearSecret}
                </button>
              ) : null}
            </div>
            <span className="settings-hint">
              {t.providerSetup.managedSecretHint(providerId, field.name)}
            </span>
            {credentialSourceHint ? <span className="settings-hint">{credentialSourceHint}</span> : null}
            {field.configured ? <span className="settings-hint">{t.providerSetup.existingCredentialHint}</span> : null}
            <input
              type="password"
              autoComplete="new-password"
              value={secretUpdates[field.name] ?? ""}
              placeholder={field.placeholder ?? (field.configured ? t.providerSetup.replaceSecret : t.providerSetup.enterSecret)}
              onChange={(event) => updateSecret(field, event.currentTarget.value)}
              aria-label={label}
            />
          </>
        ) : field.kind === "select" ? (
          <select
            value={values[field.name] ?? ""}
            onChange={(event) => {
              const value = event.currentTarget.value;
              setValues((current) => ({ ...current, [field.name]: value }));
            }}
            aria-label={label}
          >
            {!field.required ? <option value="">—</option> : null}
            {field.options?.map((option) => <option key={option} value={option}>{option}</option>)}
          </select>
        ) : (
          <input
            type={field.kind === "number" ? "number" : "text"}
            value={values[field.name] ?? ""}
            placeholder={field.placeholder ?? undefined}
            onChange={(event) => {
              const value = event.currentTarget.value;
              setValues((current) => ({ ...current, [field.name]: value }));
            }}
            aria-label={label}
          />
        )}
        {field.description ? <span className="settings-hint">{field.description}</span> : null}
        {field.helpUrl ? <a href={field.helpUrl} target="_blank" rel="noreferrer">{t.providerSetup.openGuide}</a> : null}
      </label>
    );
  }

  if (!descriptor) {
    return <section className="provider-subpage" data-testid="provider-setup-page">{message ?? t.settings.loading}</section>;
  }

  const primaryLabel = t.providerSetup.saveAndTest;

  return (
    <section className="provider-subpage provider-setup" data-testid="provider-setup-page">
      <div className="settings-section-title provider-subpage__title">
        <div>
          <h3>{t.providerSetup.title(descriptor.displayName)}</h3>
          <span className={`provider-setup__state provider-setup__state--${descriptor.setupState}`}>
            {descriptor.setupState === "ready"
              ? t.providerSetup.stateReady
              : descriptor.setupState === "unverified"
                ? t.providerSetup.stateUnverified
                : t.providerSetup.statePending}
          </span>
        </div>
        <button type="button" className="button-secondary" onClick={() => requestLeave("back")} disabled={isBusy}>
          {t.providerSetup.back}
        </button>
      </div>
      <div className="provider-setup__form">
        <label className="provider-setup__field">
          <span>{t.providerSetup.accountName} <span className="provider-setup__required">{t.providerSetup.required}</span></span>
          <input value={displayName} onChange={(event) => setDisplayName(event.currentTarget.value)} aria-label={t.providerSetup.accountName} />
        </label>
        {visibleFields.map(renderField)}
      </div>
      {hasAdvancedFields ? (
        <button type="button" className="button-secondary" onClick={() => setShowAdvanced((current) => !current)}>
          {t.providerSetup.advancedSettings}
        </button>
      ) : null}
      {message ? <div className="settings-message" role="status">{message}</div> : null}
      {result?.errorCategory ? (
        <div className="settings-message provider-setup__error-category">
          {t.providerSetup.errorCategory(result.errorCategory)}
        </div>
      ) : null}
      {result?.provider && result.success ? (
        <div className="settings-message provider-setup__preview">
          {t.providerSetup.providerPreview(result.provider.windows.length)}
        </div>
      ) : null}
      <div className="settings-actions provider-setup__actions">
        <button type="button" className="button-secondary" onClick={() => void save(false)} disabled={isBusy}>
          {t.providerSetup.saveWithoutTesting}
        </button>
        <button type="button" className="button-primary" onClick={() => void save(true)} disabled={isBusy}>
          {isBusy ? t.providerSetup.testing : primaryLabel}
        </button>
        {result?.success ? (
          <button type="button" onClick={onComplete}>{t.providerSetup.complete}</button>
        ) : null}
      </div>
      {leaveTarget ? (
        <div className="dialog-overlay" role="presentation">
          <section className="dialog" role="dialog" aria-modal="true" aria-labelledby="provider-setup-unsaved-title">
            <h3 id="provider-setup-unsaved-title">{t.settings.unsavedChangesTitle}</h3>
            <p>{t.settings.unsavedChangesPrompt}</p>
            <div className="dialog-actions">
              <button type="button" className="button-secondary" onClick={() => setLeaveTarget(null)}>
                {t.settings.cancel}
              </button>
              <button
                type="button"
                className="button-danger"
                onClick={() => {
                  const target = leaveTarget;
                  setSecretUpdates({});
                  setLeaveTarget(null);
                  if (target === "close") {
                    onRequestClose();
                  } else {
                    onBack();
                  }
                }}
              >
                {t.settings.discardChanges}
              </button>
              <button type="button" onClick={() => void saveAndLeave()} disabled={isBusy}>
                {t.settings.saveAndContinue}
              </button>
            </div>
          </section>
        </div>
      ) : null}
    </section>
  );
}
