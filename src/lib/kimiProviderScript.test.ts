/// <reference types="node" />

import { execFileSync } from "node:child_process";
import { mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterEach, expect, test } from "vitest";

type ScriptWindow = {
  id: string;
  label: string;
  remaining?: number | null;
  used: number | null;
  limit: number | null;
  usedPercent: number | null;
  remainingPercent: number | null;
  confidence: "exact" | "estimated" | "unknown";
};

type ScriptSnapshot = {
  windows: ScriptWindow[];
};

const tempDirs: string[] = [];

afterEach(() => {
  for (const dir of tempDirs.splice(0)) {
    rmSync(dir, { recursive: true, force: true });
  }
});

test("kimi_provider_script_treats_unknown_five_hour_usage_as_full_remaining", () => {
  const snapshot = runKimiProvider({
    user: { region: "REGION_CN", membership: { level: "LEVEL_INTERMEDIATE" } },
    usage: {
      limit: "100",
      used: "15",
      remaining: "85",
      resetTime: "2026-06-12T02:35:14.207781Z",
    },
    limits: [
      {
        window: { duration: 300, timeUnit: "TIME_UNIT_MINUTE" },
        detail: {
          used: "",
          remaining: "",
          limit: "",
          resetTime: "2026-06-07T16:35:14.207781Z",
        },
      },
    ],
  });

  const fiveHour = snapshot.windows.find((window) => window.id === "300-minute");
  expect(fiveHour).toMatchObject({
    label: "5h",
    used: null,
    remaining: null,
    limit: null,
    usedPercent: 0,
    remainingPercent: 100,
    confidence: "estimated",
  });
});

test("kimi_provider_script_does_not_create_missing_five_hour_window", () => {
  const snapshot = runKimiProvider({
    usage: {
      limit: "100",
      used: "15",
      remaining: "85",
      resetTime: "2026-06-12T02:35:14.207781Z",
    },
    limits: [
      {
        window: { duration: 60, timeUnit: "TIME_UNIT_MINUTE" },
        detail: { used: "", remaining: "", limit: "" },
      },
    ],
  });

  expect(snapshot.windows.some((window) => window.id === "300-minute")).toBe(false);
  expect(snapshot.windows.find((window) => window.id === "usage")).toMatchObject({
    usedPercent: 15,
    remainingPercent: 85,
    confidence: "exact",
  });
});

function runKimiProvider(raw: unknown): ScriptSnapshot {
  const dir = mkdtempSync(join(tmpdir(), "quotabarwin-kimi-"));
  tempDirs.push(dir);
  const fixturePath = join(dir, "kimi.json");
  writeFileSync(fixturePath, JSON.stringify(raw), "utf8");

  const stdout = execFileSync(
    process.execPath,
    [join(process.cwd(), "examples", "remote-providers", "kimi-coding", "provider.cjs")],
    {
      cwd: process.cwd(),
      encoding: "utf8",
      env: {
        ...process.env,
        QUOTABARWIN_KIMI_FIXTURE: fixturePath,
      },
    },
  );

  return JSON.parse(stdout) as ScriptSnapshot;
}
