import assert from "node:assert/strict";
import fs from "node:fs";
import net from "node:net";
import os from "node:os";
import path from "node:path";
import { spawn, spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import { remote } from "webdriverio";
import { download as downloadEdgeDriver } from "edgedriver";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const isWindows = process.platform === "win32";
const npmCommand = isWindows ? "npm.cmd" : "npm";
const appExecutable = path.join(
  root,
  "src-tauri",
  "target",
  "release",
  isWindows ? "quotabarwin.exe" : "quotabarwin"
);
const tauriDriver = resolveTauriDriver();
const nativeDriver = process.env.MSEDGEDRIVER ?? await downloadEdgeDriver();
const port = Number(process.env.TAURI_DRIVER_PORT ?? 4444);
const portableDir = path.dirname(appExecutable);
const configPath = path.join(portableDir, "config.quotaBarWin.json");
const portableMarkerPath = path.join(portableDir, "quotabarwin.portable");

let driverProcess;
let app;

try {
  run(npmCommand, ["run", "build"], root);
  run("cargo", ["build", "--release", "--features", "tauri/custom-protocol"], path.join(root, "src-tauri"));
  assertBuiltAppExists();
  writeE2eConfig();
  assertE2eConfigWritten();

  driverProcess = spawn(tauriDriver, ["--port", String(port), "--native-driver", nativeDriver], {
    env: process.env,
    stdio: "inherit"
  });
  await waitForPort(port);

  app = await remote({
    logLevel: "error",
    hostname: "127.0.0.1",
    port,
    path: "/",
    capabilities: {
      "tauri:options": {
        application: appExecutable
      }
    }
  });

  await assertAppStartedAndShowsQuota();
  await assertConfigCanBeSaved();
  await assertCliProviderCanRunAndTimeout();
  await assertPermissionPromptNamesProvider();

  console.log("Tauri WebdriverIO E2E passed");
} finally {
  if (app) {
    await app.deleteSession().catch(() => undefined);
  }
  if (driverProcess && !driverProcess.killed) {
    driverProcess.kill();
  }
  cleanupE2eConfig();
}

async function assertAppStartedAndShowsQuota() {
  const title = await app.$("h1");
  await title.waitForDisplayed({ timeout: 20000 });
  assert.equal(await title.getText(), "QuotaBarWin");

  const status = await byTestId("global-status-strip");
  await status.waitForDisplayed({ timeout: 20000 });
  let statusText = await status.getText();
  await app.waitUntil(
    async () => {
      statusText = await status.getText();
      return /providers active|needs attention|refresh failed/.test(statusText);
    },
    {
      timeout: 20000,
      timeoutMsg: "Quota status did not finish loading"
    }
  ).catch(async (error) => {
    await logConfigStorageDiagnostics();
    throw error;
  });
  assert.match(statusText, /providers active|needs attention|refresh failed/);

  const mockProvider = await byTestId("provider-card-mock-codex");
  await mockProvider.waitForDisplayed({ timeout: 20000 });
  const quotaRow = await byTestId("quota-row-mock-codex-5h");
  assert.equal(await quotaRow.isDisplayed(), true);
}

async function logConfigStorageDiagnostics() {
  try {
    await openSettings();
    const advanced = await byTestId("advanced-settings-section");
    if (!(await advanced.getAttribute("open"))) {
      await advanced.$("summary").then((summary) => summary.click());
    }
    const storage = await app.$('[aria-label="Configuration storage"]');
    if (!(await storage.getAttribute("open"))) {
      await storage.$("summary").then((summary) => summary.click());
    }
    console.error("E2E config diagnostics:");
    console.error(`  expected config: ${configPath}`);
    console.error(`  expected marker: ${portableMarkerPath}`);
    if (fs.existsSync(configPath)) {
      const config = JSON.parse(fs.readFileSync(configPath, "utf8"));
      console.error(`  config providers: ${config.providers?.length ?? 0}`);
    }
    console.error(`  UI storage text: ${await storage.getText()}`);
  } catch (error) {
    console.error(`Unable to read config diagnostics: ${error?.message ?? error}`);
  }
}

function assertE2eConfigWritten() {
  const written = JSON.parse(fs.readFileSync(configPath, "utf8"));
  if (written.providers?.length !== 3) {
    throw new Error(`E2E config was not written correctly at ${configPath}`);
  }
  console.log(`E2E config seeded at ${configPath}`);
}

async function assertConfigCanBeSaved() {
  await openSettings();
  const input = await byTestId("refresh-interval-input");
  await input.setValue("120");

  const save = await byTestId("save-settings-button");
  await app.waitUntil(async () => save.isEnabled(), {
    timeout: 5000,
    timeoutMsg: "Save button did not become enabled"
  });
  await clickByTestId("save-settings-button");
  await byTestId("overview-page").then((overview) => overview.waitForDisplayed({ timeout: 20000 }));

  const saved = JSON.parse(fs.readFileSync(configPath, "utf8"));
  assert.equal(saved.refreshIntervalSeconds, 120);
}

async function assertCliProviderCanRunAndTimeout() {
  await openSettings();

  await clickByTestId("edit-provider-fixture-cli");
  await clickByTestId("test-provider-fixture-cli");
  await app.waitUntil(async () => {
    const output = await providerOutputText("fixture-cli");
    return output.includes("Fake Command Provider");
  }, {
    timeout: 10000,
    timeoutMsg: "Fixture CLI provider did not return quota output"
  });

  await clickByTestId("edit-provider-slow-cli");
  await clickByTestId("test-provider-slow-cli");
  await app.waitUntil(async () => {
    const output = await providerOutputText("slow-cli");
    return output.includes("timedOut") && output.includes("true");
  }, {
    timeout: 10000,
    timeoutMsg: "Slow CLI provider did not show timeout interruption"
  });
}

async function assertPermissionPromptNamesProvider() {
  await clickByTestId("more-provider-fixture-cli");
  await clickByTestId("remove-provider-fixture-cli");

  const alertText = await app.getAlertText();
  assert.match(alertText, /Fixture CLI/);
  await app.dismissAlert();
}

async function openSettings() {
  const settingsPage = await byTestId("settings-page");
  if (await settingsPage.isExisting()) {
    return;
  }
  const settingsButton = await app.$('//button[normalize-space(.)="Settings"]');
  await app.execute((target) => target.click(), settingsButton);
  await settingsPage.waitForDisplayed({ timeout: 10000 });
}

async function byTestId(id) {
  return app.$(`[data-testid="${id}"]`);
}

async function clickByTestId(id) {
  const element = await byTestId(id);
  await element.waitForDisplayed({ timeout: 10000 });
  await app.execute((target) => {
    target.scrollIntoView({ block: "center", inline: "nearest" });
    target.click();
  }, element);
}

async function providerOutputText(providerId) {
  const output = await app.$(`[data-testid="settings-provider-${providerId}"] pre`);
  if (!(await output.isExisting())) {
    return "";
  }
  return output.getText();
}

function writeE2eConfig() {
  fs.mkdirSync(path.dirname(configPath), { recursive: true });
  fs.writeFileSync(portableMarkerPath, "QuotaBarWin E2E portable mode\n");
  const fixture = (name) => path.join(root, "fixtures", name);
  fs.writeFileSync(
    configPath,
    JSON.stringify(
      {
        schemaVersion: 6,
        refreshIntervalSeconds: 300,
        displayMode: "remaining",
        lowQuotaWarningThreshold: 20,
        launchAtStartup: false,
        logLevel: "info",
        providers: [
          {
            kind: "mock",
            id: "mock-codex",
            name: "Codex Mock",
            enabled: true
          },
          {
            kind: "script",
            id: "fixture-cli",
            name: "Fixture CLI",
            enabled: true,
            command: {
              executable: "node",
              args: [fixture("fake_provider_snapshot.js")],
              cwd: null,
              env: {},
              timeoutMs: 2000
            },
            output: { type: "provider-snapshot-v1" },
            windowLabelOverrides: {},
            visibleWindowIds: []
          },
          {
            kind: "script",
            id: "slow-cli",
            name: "Slow CLI",
            enabled: false,
            command: {
              executable: "node",
              args: [fixture("fake_slow.js")],
              cwd: null,
              env: {},
              timeoutMs: 50
            },
            output: { type: "provider-snapshot-v1" },
            windowLabelOverrides: {},
            visibleWindowIds: []
          }
        ]
      },
      null,
      2
    )
  );
}

function cleanupE2eConfig() {
  for (const file of [configPath, portableMarkerPath]) {
    try {
      fs.rmSync(file, { force: true });
    } catch {
      // best-effort cleanup only
    }
  }
}

function resolveTauriDriver() {
  const candidates = [
    process.env.TAURI_DRIVER,
    path.join(os.homedir(), ".cargo", "bin", isWindows ? "tauri-driver.exe" : "tauri-driver"),
    "tauri-driver"
  ].filter(Boolean);

  for (const candidate of candidates) {
    if (candidate.includes(path.sep) && fs.existsSync(candidate)) {
      return candidate;
    }
    const check = spawnSync(isWindows ? "where.exe" : "which", [candidate], { encoding: "utf8" });
    if (check.status === 0) {
      return check.stdout.trim().split(/\r?\n/)[0];
    }
  }

  throw new Error("tauri-driver was not found. Install it with: cargo install tauri-driver --locked");
}

function run(command, args, cwd) {
  const result =
    isWindows && command === npmCommand
      ? spawnSync([command, ...args].join(" "), { cwd, shell: true, stdio: "inherit" })
      : spawnSync(command, args, { cwd, stdio: "inherit" });
  if (result.status !== 0) {
    throw new Error(`${command} ${args.join(" ")} failed`);
  }
}

async function waitForPort(targetPort) {
  const deadline = Date.now() + 20000;
  while (Date.now() < deadline) {
    if (await canConnect(targetPort)) {
      return;
    }
    await new Promise((resolve) => setTimeout(resolve, 200));
  }
  throw new Error(`tauri-driver did not open port ${targetPort}`);
}

function assertBuiltAppExists() {
  if (!fs.existsSync(appExecutable)) {
    throw new Error(`Built app was not found at ${appExecutable}`);
  }
}

function canConnect(targetPort) {
  return new Promise((resolve) => {
    const socket = net.createConnection({ host: "127.0.0.1", port: targetPort });
    socket.once("connect", () => {
      socket.destroy();
      resolve(true);
    });
    socket.once("error", () => {
      socket.destroy();
      resolve(false);
    });
  });
}
