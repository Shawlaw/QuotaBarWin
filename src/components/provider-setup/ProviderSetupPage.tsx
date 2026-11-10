import { useEffect, useMemo, useRef, useState } from "react";
import {
  adoptParameterAsManagedSecret,
  copyManagedSecret,
  getConfig,
  getProviderSetup,
  openExternalLink,
  revealManagedSecret,
  saveProviderSetup,
  testProviderSetup,
} from "../../lib/api";
import { mutationMessage, type MutationResult } from "../../lib/configMutation";
import type { AppConfig } from "../../types";
import type {
  ProviderSetupDescriptor,
  ProviderSetupField,
  ProviderSetupTestResult,
} from "../../types";
import { useI18n } from "../../i18n";

type SetupMutation = <T>(
  operation: () => Promise<{ config: AppConfig; result: T; warnings?: string[] }>
) => Promise<MutationResult<T>>;

type ProviderSetupPageProps = {
  providerId: string;
  onBack: () => void;
  onComplete: () => void;
  onConfigChanged: (testResult?: ProviderSetupTestResult) => Promise<void>;
  closeRequest: number;
  onRequestClose: () => void;
  /** App-owned single-flight mutation entry; classifies failures and
   * recovers from uncertain outcomes (config may have been committed). */
  runMutation?: SetupMutation;
  /** True while the app requires a config reload: all writes stay disabled. */
  blocked?: boolean;
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
  if (field.kind === "secret") {
    return false;
  }
  return field.advanced === true || !field.required;
}

export function ProviderSetupPage({
  providerId,
  onBack,
  onComplete,
  onConfigChanged,
  closeRequest,
  onRequestClose,
  runMutation,
  blocked = false,
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
  const [adoptingField, setAdoptingField] = useState<string | null>(null);
  const [revealedSecrets, setRevealedSecrets] = useState<Record<string, string>>({});

  // A descriptor refresh means the authoritative field state changed; any
  // revealed plaintext is stale and must leave memory immediately.
  useEffect(() => {
    setRevealedSecrets({});
  }, [descriptor]);
  const [leaveTarget, setLeaveTarget] = useState<"back" | "close" | null>(null);
  // A close request is an event. The setup page can mount after a previous
  // settings navigation, so it must not treat that historic value as a new
  // request to leave the configuration flow.
  const handledCloseRequestRef = useRef(closeRequest);

  const visibleFields = useMemo(
    () =>
      descriptor?.fields
        .filter((field) => field.kind !== "secret")
        .filter((field) => showAdvanced || !isAdvanced(field)) ?? [],
    [descriptor, showAdvanced],
  );
  const requiredCredentialFields = useMemo(
    () =>
      descriptor?.fields.filter((field) => field.kind === "secret" && field.required) ?? [],
    [descriptor],
  );
  const optionalCredentialFields = useMemo(
    () =>
      descriptor?.fields.filter((field) => field.kind === "secret" && !field.required) ?? [],
    [descriptor],
  );
  const hasAdvancedFields = useMemo(
    () => descriptor?.fields.some((field) => field.kind !== "secret" && isAdvanced(field)) ?? false,
    [descriptor],
  );
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
        if (update === null) {
          // Explicit clear is a valid save: it removes the binding and leaves
          // the Provider disabled until the credential is entered again.
          continue;
        }
        const configured = update === undefined ? field.configured : update.trim().length > 0;
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

  /**
   * Runs a setup mutation through the App-owned wrapper when available.
   * Fallback (tests / browser preview): run the command and report success;
   * command errors propagate to the caller's catch — the legacy behavior.
   */
  async function runSetupMutation<T>(
    operation: () => Promise<{ config: AppConfig; result: T; warnings?: string[] }>,
  ): Promise<MutationResult<T>> {
    if (runMutation) {
      return runMutation(operation);
    }
    const envelope = await operation();
    return { status: "success", result: envelope.result, warnings: envelope.warnings ?? [] };
  }

  function mutationOutcomeMessage(result: MutationResult<unknown>): string {
    switch (result.status) {
      case "validation":
      case "conflict":
      case "stale":
      case "busy":
      case "blocked":
        return result.message;
      case "recovered":
        return t.settings.reloadedAfterUncertain;
      case "success":
        return "";
    }
  }

  async function save(runTest: boolean): Promise<boolean> {
    if (blocked) {
      setMessage(t.settings.reloadRequired);
      return false;
    }
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
      const request = {
        providerId,
        displayName: displayName.trim(),
        values: serializedValues,
        secretUpdates,
      };

      const saved = await runSetupMutation(async () => {
        const nextDescriptor = await saveProviderSetup(request);
        const config = await getConfig();
        return { config, result: nextDescriptor };
      });
      if (saved.status !== "success") {
        if (saved.status === "recovered") {
          await loadSetup();
        }
        setMessage(mutationOutcomeMessage(saved) || t.providerSetup.saveFailed);
        return false;
      }

      // The request is intentionally the only time this component holds a
      // secret. Immediately clear it before doing any follow-up work.
      setSecretUpdates({});
      // Sync the settings baseline (snapshot merge for the test result) and
      // reset this page from the authoritative state.
      await onConfigChanged();
      if (!runTest) {
        await loadSetup();
        return true;
      }

      const tested = await runSetupMutation(async () => {
        const testResult = await testProviderSetup(providerId);
        const config = await getConfig();
        return { config, result: testResult };
      });
      if (tested.status !== "success") {
        if (tested.status === "recovered") {
          await loadSetup();
        }
        setMessage(mutationOutcomeMessage(tested) || t.providerSetup.saveFailed);
        return false;
      }
      setResult(tested.result);
      await onConfigChanged(tested.result);
      await loadSetup();
      setMessage(
        tested.result.success
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
    setSecretUpdates((current) => {
      const next = { ...current };
      if (value.trim()) {
        next[field.name] = value;
      } else {
        delete next[field.name];
      }
      return next;
    });
  }

  function clearSecret(field: ProviderSetupField) {
    setSecretUpdates((current) => ({ ...current, [field.name]: null }));
  }

  function hideRevealedSecret(field: ProviderSetupField) {
    setRevealedSecrets((current) => {
      const next = { ...current };
      delete next[field.name];
      return next;
    });
  }

  async function revealSecret(field: ProviderSetupField) {
    if (isBusy || blocked) {
      return;
    }
    setIsBusy(true);
    setMessage(null);
    try {
      const outcome = await revealManagedSecret(providerId, field.name);
      setRevealedSecrets((current) => ({ ...current, [field.name]: outcome.value }));
    } catch (error) {
      setMessage(t.providerSetup.revealFailed(mutationMessage(error, "")));
    } finally {
      setIsBusy(false);
    }
  }

  async function copySecret(field: ProviderSetupField) {
    if (isBusy || blocked) {
      return;
    }
    setIsBusy(true);
    setMessage(null);
    try {
      await copyManagedSecret(providerId, field.name);
      setMessage(t.providerSetup.copied);
    } catch (error) {
      setMessage(t.providerSetup.copyFailed(mutationMessage(error, "")));
    } finally {
      setIsBusy(false);
    }
  }

  async function adoptSecret(field: ProviderSetupField) {
    if (isBusy || blocked) {
      return;
    }
    setIsBusy(true);
    setAdoptingField(field.name);
    setMessage(null);
    try {
      const outcome = await runSetupMutation(async () => {
        const result = await adoptParameterAsManagedSecret(providerId, field.name);
        // The adopt command does not return the config; fetch the
        // authoritative state so the wrapper can commit it.
        const config = await getConfig();
        return { config, result };
      });
      if (outcome.status !== "success") {
        if (outcome.status === "recovered") {
          await loadSetup();
        }
        setMessage(mutationOutcomeMessage(outcome) || t.providerSetup.adoptFailed(""));
        return;
      }
      // With the App wrapper the config commit already happened, so the next
      // settings save cannot write the old literal binding back. The fallback
      // path (tests / browser preview) syncs the parent the legacy way.
      if (!runMutation) {
        await onConfigChanged();
      }
      setDescriptor(outcome.result.descriptor);
      setMessage(outcome.result.adopted ? t.providerSetup.adopted : null);
    } catch (error) {
      setMessage(t.providerSetup.adoptFailed(mutationMessage(error, "")));
    } finally {
      setAdoptingField(null);
      setIsBusy(false);
    }
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
    // Action buttons must not live inside a <label>: the label's implicit
    // control is its first labelable descendant (the Clear button), which
    // pollutes that button's accessible name with the whole field text and
    // forwards title clicks to it. Use a plain wrapper and associate the
    // title with the control through htmlFor/id instead.
    const inputId = `provider-setup-input-${providerId.replace(/\s+/g, "-")}-${field.name}`;
    return (
      <div className="provider-setup__field" key={field.name}>
        <label className="provider-setup__field-title" htmlFor={inputId}>
          {label} {required}
        </label>
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
            {field.configured && field.source === "literal" ? (
              <div className="provider-setup__actions">
                <button
                  type="button"
                  className="button-secondary button-compact"
                  disabled={isBusy}
                  data-testid={`adopt-secret-${field.name}`}
                  onClick={() => void adoptSecret(field)}
                >
                  {adoptingField === field.name
                    ? t.providerSetup.adopting
                    : t.providerSetup.adoptSecret}
                </button>
                <span className="settings-hint">{t.providerSetup.adoptHint}</span>
              </div>
            ) : null}
            {field.configured && field.source === "managedLocalFile" ? (
              <div className="provider-setup__actions">
                {revealedSecrets[field.name] !== undefined ? (
                  <>
                    <code data-testid={`revealed-secret-${field.name}`}>
                      {revealedSecrets[field.name]}
                    </code>
                    <button
                      type="button"
                      className="button-ghost"
                      onClick={() => hideRevealedSecret(field)}
                    >
                      {t.providerSetup.hideSecret}
                    </button>
                  </>
                ) : (
                  <>
                    <button
                      type="button"
                      className="button-secondary button-compact"
                      disabled={isBusy}
                      data-testid={`reveal-secret-${field.name}`}
                      onClick={() => void revealSecret(field)}
                    >
                      {t.providerSetup.revealSecret}
                    </button>
                  </>
                )}
                <button
                  type="button"
                  className="button-secondary button-compact"
                  disabled={isBusy}
                  data-testid={`copy-secret-${field.name}`}
                  onClick={() => void copySecret(field)}
                >
                  {t.providerSetup.copySecret}
                </button>
              </div>
            ) : null}
            {field.configured ? <span className="settings-hint">{t.providerSetup.existingCredentialHint}</span> : null}
            <input
              type="password"
              autoComplete="new-password"
              id={inputId}
              value={secretUpdates[field.name] ?? ""}
              placeholder={field.placeholder ?? (field.configured ? t.providerSetup.replaceSecret : t.providerSetup.enterSecret)}
              onChange={(event) => updateSecret(field, event.currentTarget.value)}
              aria-label={label}
            />
          </>
        ) : field.kind === "select" ? (
          <select
            id={inputId}
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
            id={inputId}
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
        {field.helpUrl ? (
          <a
            href={field.helpUrl}
            target="_blank"
            rel="noreferrer"
            onClick={(event) => {
              // The WebView does not route target="_blank" to the system
              // browser, so guide links go through the external-link command.
              event.preventDefault();
              void openExternalLink(field.helpUrl ?? "");
            }}
          >
            {t.providerSetup.openGuide}
          </a>
        ) : null}
      </div>
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
        {requiredCredentialFields.map(renderField)}
        {optionalCredentialFields.length > 0 ? (
          <details className="provider-setup__optional-group">
            <summary>
              {t.providerSetup.optionalCredentials(optionalCredentialFields.length)}
            </summary>
            {optionalCredentialFields.map(renderField)}
          </details>
        ) : null}
        {(descriptor.unknownEnvVars?.length ?? 0) > 0 ? (
          <div className="provider-setup__unknown-vars" data-testid="provider-setup-unknown-vars">
            <span>{t.providerSetup.unknownEnvVars}</span>
            <code>{descriptor.unknownEnvVars?.join(", ")}</code>
            <span className="settings-hint">{t.providerSetup.unknownEnvVarsHint}</span>
          </div>
        ) : null}
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
