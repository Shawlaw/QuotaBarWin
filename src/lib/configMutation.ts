import type { AppConfig, MutationFailure } from "../types";

/** Authoritative reset of the app's config state after a mutation. */
export type MutationEnvelope<T> = {
  config: AppConfig;
  result: T;
  warnings?: string[];
};

export type MutationResult<T> =
  | { status: "success"; result: T; warnings: string[] }
  | { status: "validation"; message: string }
  | { status: "conflict"; message: string; fields: string[] }
  | { status: "stale"; message: string }
  | { status: "busy"; message: string }
  | { status: "recovered"; message: string; config: AppConfig }
  | { status: "blocked"; message: string };

export type MutationGuard = {
  tryAcquire(): boolean;
  release(): void;
  isLocked(): boolean;
};

export type MutationMessages = {
  /** Shown when a second mutation is attempted while one is in flight. */
  busy: string;
  /** Shown after an uncertain mutation was resolved by reloading. */
  recovered: string;
  /** Shown when mutations are blocked until a reload succeeds. */
  reloadRequired: string;
};

export type MutationDependencies = {
  commitConfig: (config: AppConfig) => void;
  reloadConfig: () => Promise<AppConfig>;
  setReloadRequired: (value: boolean) => void;
  /** Synchronous check so blocked mutations are rejected before any IPC. */
  isReloadRequired?: () => boolean;
  /** Localized strings; the English defaults are only a safety net. */
  messages?: Partial<MutationMessages>;
};

export function createMutationGuard(): MutationGuard {
  let locked = false;
  return {
    tryAcquire() {
      if (locked) {
        return false;
      }
      locked = true;
      return true;
    },
    release() {
      locked = false;
    },
    isLocked() {
      return locked;
    },
  };
}

export function isMutationFailure(value: unknown): value is MutationFailure {
  if (!value || typeof value !== "object") {
    return false;
  }
  const kind = (value as { kind?: unknown }).kind;
  return (
    kind === "validation" ||
    kind === "conflict" ||
    kind === "stale" ||
    kind === "uncertain" ||
    kind === "io"
  );
}

export function mutationMessage(value: unknown, fallback: string): string {
  if (typeof value === "string" && value.trim()) {
    return value;
  }
  if (value && typeof value === "object" && "message" in value) {
    const message = (value as { message?: unknown }).message;
    if (typeof message === "string" && message.trim()) {
      return message;
    }
  }
  if (value instanceof Error && value.message) {
    return value.message;
  }
  return fallback;
}

/**
 * Runs one config mutation with a single-flight guard.
 *
 * - success: commits the authoritative config;
 * - validation/conflict/stale: definitely not committed, caller keeps drafts;
 * - uncertain/io/transport errors: reload once; a successful reload is
 *   `recovered`, a failed one blocks further mutations via
 *   `setReloadRequired(true)`.
 */
export async function runConfigMutation<T>(
  operation: () => Promise<MutationEnvelope<T>>,
  dependencies: MutationDependencies,
  guard: MutationGuard,
): Promise<MutationResult<T>> {
  const messages = {
    busy: "Another configuration change is still running",
    recovered: "Configuration reloaded",
    reloadRequired: "Reload the configuration before continuing",
    ...dependencies.messages,
  };

  // Blocked mutations are rejected before any IPC so a stale draft can never
  // be written back while the app is waiting for a successful reload.
  if (dependencies.isReloadRequired?.()) {
    return { status: "blocked", message: messages.reloadRequired };
  }
  if (!guard.tryAcquire()) {
    return { status: "busy", message: messages.busy };
  }

  try {
    const envelope = await operation();
    dependencies.commitConfig(envelope.config);
    return {
      status: "success",
      result: envelope.result,
      warnings: envelope.warnings ?? [],
    };
  } catch (error) {
    if (isMutationFailure(error)) {
      switch (error.kind) {
        case "validation":
          return { status: "validation", message: error.message };
        case "conflict":
          return {
            status: "conflict",
            message: error.message,
            fields: error.fields ?? [],
          };
        case "stale":
          return { status: "stale", message: error.message };
        case "uncertain":
        case "io":
          break;
      }
    }

    try {
      const reloaded = await dependencies.reloadConfig();
      dependencies.commitConfig(reloaded);
      return {
        status: "recovered",
        message: mutationMessage(error, messages.recovered),
        config: reloaded,
      };
    } catch (reloadError) {
      dependencies.setReloadRequired(true);
      return {
        status: "blocked",
        message: mutationMessage(reloadError, messages.reloadRequired),
      };
    }
  } finally {
    guard.release();
  }
}
