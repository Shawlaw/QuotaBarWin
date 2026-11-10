import { describe, expect, test, vi } from "vitest";
import type { AppConfig } from "../types";
import {
  createMutationGuard,
  isMutationFailure,
  mutationMessage,
  runConfigMutation,
} from "./configMutation";

const baseConfig: AppConfig = {
  schemaVersion: 29,
  refreshIntervalSeconds: 300,
  displayMode: "remaining",
  lowQuotaWarningThreshold: 20,
  language: "system",
  providers: [],
};

function dependencies() {
  return {
    commitConfig: vi.fn(),
    reloadConfig: vi.fn(async () => baseConfig),
    setReloadRequired: vi.fn(),
  };
}

describe("mutation guard", () => {
  test("is single-flight until released", () => {
    const guard = createMutationGuard();
    expect(guard.tryAcquire()).toBe(true);
    expect(guard.tryAcquire()).toBe(false);
    expect(guard.isLocked()).toBe(true);
    guard.release();
    expect(guard.isLocked()).toBe(false);
    expect(guard.tryAcquire()).toBe(true);
  });
});

describe("runConfigMutation", () => {
  test("success commits the authoritative config and returns warnings", async () => {
    const deps = dependencies();
    const guard = createMutationGuard();
    const result = await runConfigMutation(
      async () => ({ config: baseConfig, result: "ok", warnings: ["tray"] }),
      deps,
      guard,
    );

    expect(result).toEqual({ status: "success", result: "ok", warnings: ["tray"] });
    expect(deps.commitConfig).toHaveBeenCalledWith(baseConfig);
    expect(deps.reloadConfig).not.toHaveBeenCalled();
    expect(guard.isLocked()).toBe(false);
  });

  test("validation keeps the caller in control and does not reload", async () => {
    const deps = dependencies();
    const result = await runConfigMutation(
      async () => {
        throw { kind: "validation", message: "bad port" };
      },
      deps,
      createMutationGuard(),
    );

    expect(result).toEqual({ status: "validation", message: "bad port" });
    expect(deps.commitConfig).not.toHaveBeenCalled();
    expect(deps.reloadConfig).not.toHaveBeenCalled();
  });

  test("conflict returns the conflicting fields", async () => {
    const deps = dependencies();
    const result = await runConfigMutation(
      async () => {
        throw {
          kind: "conflict",
          message: "changed elsewhere",
          fields: ["providers[kimi].envVars"],
        };
      },
      deps,
      createMutationGuard(),
    );

    expect(result).toEqual({
      status: "conflict",
      message: "changed elsewhere",
      fields: ["providers[kimi].envVars"],
    });
  });

  test("uncertain reloads once and reports recovered", async () => {
    const deps = dependencies();
    const reloaded = { ...baseConfig, refreshIntervalSeconds: 60 };
    deps.reloadConfig.mockResolvedValue(reloaded);
    const result = await runConfigMutation(
      async () => {
        throw { kind: "uncertain", message: "lost response", rollbackFailed: false };
      },
      deps,
      createMutationGuard(),
    );

    expect(result.status).toBe("recovered");
    expect(deps.reloadConfig).toHaveBeenCalledTimes(1);
    expect(deps.commitConfig).toHaveBeenCalledWith(reloaded);
    expect(deps.setReloadRequired).not.toHaveBeenCalled();
  });

  test("a failed reload blocks further mutations", async () => {
    const deps = dependencies();
    deps.reloadConfig.mockRejectedValue(new Error("ipc down"));
    const result = await runConfigMutation(
      async () => {
        throw { kind: "io", message: "write failed" };
      },
      deps,
      createMutationGuard(),
    );

    expect(result.status).toBe("blocked");
    expect(deps.setReloadRequired).toHaveBeenCalledWith(true);
  });

  test("rejects a second mutation while one is running", async () => {
    const deps = dependencies();
    const guard = createMutationGuard();
    guard.tryAcquire();
    const result = await runConfigMutation(
      async () => ({ config: baseConfig, result: "ok" }),
      deps,
      guard,
    );

    expect(result.status).toBe("busy");
    expect(deps.commitConfig).not.toHaveBeenCalled();
    guard.release();
  });
});

describe("failure helpers", () => {
  test("classifies structured failures only", () => {
    expect(isMutationFailure({ kind: "conflict", message: "x" })).toBe(true);
    expect(isMutationFailure(new Error("x"))).toBe(false);
    expect(isMutationFailure({ kind: "other" })).toBe(false);
  });

  test("extracts the most useful message", () => {
    expect(mutationMessage({ message: "from object" }, "fallback")).toBe("from object");
    expect(mutationMessage(new Error("from error"), "fallback")).toBe("from error");
    expect(mutationMessage(undefined, "fallback")).toBe("fallback");
  });
});
