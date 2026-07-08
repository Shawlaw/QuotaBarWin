import assert from "node:assert/strict";
import { createHash } from "node:crypto";
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
const logPath = path.join(portableDir, "quotabarwin.log");
const portableMarkerPath = path.join(portableDir, "quotabarwin.portable");
const e2eProviderRoot = path.join(portableDir, "providers", "remote");
const trayPopupLabel = "tray-popup";

let driverProcess;
let app;

try {
  assertNoConflictingAppInstance();
  run(npmCommand, ["run", "build"], root);
  run("cargo", ["build", "--release", "--features", "tauri/custom-protocol"], path.join(root, "src-tauri"));
  assertBuiltAppExists();
  writeE2eConfig();
  assertE2eConfigWritten();

  driverProcess = spawn(tauriDriver, ["--port", String(port), "--native-driver", nativeDriver], {
    env: { ...process.env, QBWIN_E2E: "1" },
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
  await assertRemoteProviderCanRefreshAndTimeout();
  await assertTrayPopupInteractions();
  await assertPermissionPromptNamesProvider();

  console.log("Tauri WebdriverIO E2E passed");
} finally {
  if (app) {
    await app.deleteSession().catch(() => undefined);
  }
  if (driverProcess && !driverProcess.killed) {
    driverProcess.kill();
    await waitForProcessExit(driverProcess, 5000);
  }
  await sleep(1000);
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

  const fixtureProvider = await byTestId("provider-card-e2e-remote-fixture");
  await fixtureProvider.waitForDisplayed({ timeout: 20000 });
  const quotaRow = await byTestId("quota-row-e2e-remote-fixture-daily");
  assert.equal(await quotaRow.isDisplayed(), true);

  const slowProvider = await byTestId("provider-card-e2e-remote-slow");
  await slowProvider.waitForDisplayed({ timeout: 20000 });
  const slowStatus = await byTestId("provider-status-e2e-remote-slow");
  assert.match(await slowStatus.getText(), /error|错误/i);
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
  if (written.schemaVersion !== 14 || written.providers?.length !== 2) {
    throw new Error(`E2E config was not written correctly at ${configPath}`);
  }
  assert.deepEqual(
    written.providers.map((provider) => provider.kind),
    ["remote", "remote"]
  );
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

async function assertRemoteProviderCanRefreshAndTimeout() {
  await clickByTestId("provider-refresh-e2e-remote-fixture");
  await app.waitUntil(async () => {
    const quotaRow = await byTestId("quota-row-e2e-remote-fixture-daily");
    return quotaRow.isDisplayed();
  }, {
    timeout: 20000,
    timeoutMsg: "Fixture remote provider did not refresh quota output"
  });

  await clickByTestId("provider-status-e2e-remote-slow");
  const slowCard = await byTestId("provider-card-e2e-remote-slow");
  await app.waitUntil(async () => {
    const text = await slowCard.getText();
    return text.includes("timed out") || text.includes("timeoutOrigin=host");
  }, {
    timeout: 10000,
    timeoutMsg: "Slow remote provider did not show timeout diagnostics"
  });
}

async function assertTrayPopupInteractions() {
  const mainHandle = await app.getWindowHandle();

  await showTrayPopupForE2e();
  const trayHandle = await switchToWindowWithTestId("tray-popup");
  await byTestId("tray-popup").then((popup) => popup.waitForDisplayed({ timeout: 10000 }));
  await app.waitUntil(async () => (await currentBodyText()).includes("E2E Remote Fixture"), {
    timeout: 20000,
    timeoutMsg: "Tray popup did not render the fixture provider"
  });
  const trayText = await currentBodyText();
  assert.match(trayText, /Daily/);
  assert.match(trayText, /Weekly limit/);

  const refresh = await byTestId("tray-popup-refresh");
  await refresh.click();
  await app.waitUntil(async () => (await currentBodyText()).includes("Daily"), {
    timeout: 20000,
    timeoutMsg: "Tray popup refresh did not keep quota output visible"
  });

  await app.switchToWindow(mainHandle);
  await invokeInApp("e2e_set_tray_popup_size", { width: 460, height: 610 });
  await showTrayPopupForE2e();
  await app.switchToWindow(trayHandle);
  await byTestId("tray-popup-titlebar").then((titlebar) => titlebar.doubleClick());
  await app.waitUntil(() => {
    const config = JSON.parse(fs.readFileSync(configPath, "utf8"));
    return config.trayPopupSize?.width === 380 && config.trayPopupSize?.height === 520;
  }, {
    timeout: 10000,
    timeoutMsg: "Tray popup titlebar double-click did not reset the saved size"
  });

  await byTestId("tray-popup-close").then((close) => close.click());
  await app.switchToWindow(mainHandle);
  await waitForTauriWindowVisible(trayPopupLabel, false, "Tray popup close button did not hide the window");

  await showTrayPopupForE2e();
  await switchToWindowWithTestId("tray-popup");
  await app.keys("Escape");
  await app.switchToWindow(mainHandle);
  await waitForTauriWindowVisible(trayPopupLabel, false, "Tray popup Escape key did not hide the window");

  await showTrayPopupForE2e();
  await switchToWindowWithTestId("tray-popup");
  await app.switchToWindow(mainHandle);
  await invokeInApp("e2e_focus_main_window");
  await waitForTauriWindowVisible(trayPopupLabel, false, "Tray popup focus loss did not hide the window");

  assertTrayPopupLogs();
}

async function assertPermissionPromptNamesProvider() {
  await openSettings();
  await clickByTestId("more-provider-e2e-remote-fixture");
  await clickByTestId("remove-provider-e2e-remote-fixture");

  const alertText = await app.getAlertText();
  assert.match(alertText, /E2E Remote Fixture/);
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

async function currentBodyText() {
  const body = await app.$("body");
  return body.getText();
}

async function invokeInApp(command, args = {}) {
  const result = await app.executeAsync((cmd, cmdArgs, done) => {
    window.__TAURI_INTERNALS__.invoke(cmd, cmdArgs).then(
      (value) => done({ ok: true, value }),
      (error) => done({ ok: false, error: String(error?.message ?? error) })
    );
  }, command, args);

  if (!result?.ok) {
    throw new Error(`Tauri invoke failed for ${command}: ${result?.error ?? "unknown error"}`);
  }

  return result.value;
}

async function showTrayPopupForE2e() {
  await invokeInApp("e2e_show_tray_popup");
}

async function switchToWindowWithTestId(id) {
  let foundHandle = null;
  await app.waitUntil(async () => {
    for (const handle of await app.getWindowHandles()) {
      await app.switchToWindow(handle);
      const element = await byTestId(id);
      if (await element.isExisting()) {
        foundHandle = handle;
        return true;
      }
    }
    return false;
  }, {
    timeout: 10000,
    timeoutMsg: `Unable to find a WebDriver window containing [data-testid="${id}"]`
  });

  return foundHandle;
}

async function waitForTauriWindowVisible(label, expected, timeoutMsg) {
  assert.equal(label, trayPopupLabel);
  await app.waitUntil(async () => {
    const visible = await invokeInApp("e2e_is_tray_popup_visible");
    return visible === expected;
  }, {
    timeout: 5000,
    timeoutMsg
  });
}

function assertTrayPopupLogs() {
  const logText = fs.readFileSync(logPath, "utf8");
  assert.match(logText, /"target":"tray"/);
  assert.match(logText, /hide command requested|focus lost event/);
  assert.match(logText, /reset size requested/);
}

function writeE2eConfig() {
  fs.mkdirSync(path.dirname(configPath), { recursive: true });
  fs.writeFileSync(portableMarkerPath, "QuotaBarWin E2E portable mode\n");
  removePathWithRetry(e2eProviderRoot);

  const fixtureProvider = createRemoteProviderCache({
    id: "e2e-remote-fixture",
    displayName: "E2E Remote Fixture",
    version: "1.0.0",
    source: fixtureRemoteProviderSource(),
    timeoutSeconds: 5
  });
  const slowProvider = createRemoteProviderCache({
    id: "e2e-remote-slow",
    displayName: "E2E Remote Slow",
    version: "1.0.0",
    source: slowRemoteProviderSource(),
    timeoutSeconds: 1
  });

  fs.writeFileSync(
    configPath,
    JSON.stringify(
      {
        schemaVersion: 14,
        refreshIntervalSeconds: 300,
        displayMode: "remaining",
        lowQuotaWarningThreshold: 20,
        launchAtStartup: false,
        logLevel: "info",
        logMaxBytes: 10 * 1024 * 1024,
        language: "en",
        networkProxy: null,
        trayPopupPosition: null,
        trayPopupSize: null,
        remoteProviderRegistry: {
          registryUrl: null,
          providerProxyUrl: null,
          autoUpdate: false,
          sources: []
        },
        providers: [fixtureProvider, slowProvider]
      },
      null,
      2
    )
  );
}

function cleanupE2eConfig() {
  if (process.env.QBWIN_E2E_KEEP_ARTIFACTS === "1") {
    console.log(`Keeping E2E artifacts under ${portableDir}`);
    return;
  }

  for (const file of [
    configPath,
    portableMarkerPath,
    path.join(portableDir, "last_snapshot.quotaBarWin.json"),
    logPath,
    path.join(portableDir, "quotabarwin.log.1")
  ]) {
    try {
      fs.rmSync(file, { force: true });
    } catch {
      // best-effort cleanup only
    }
  }
  removePathBestEffort(e2eProviderRoot);
}

function removePathWithRetry(targetPath) {
  fs.rmSync(targetPath, {
    recursive: true,
    force: true,
    maxRetries: 20,
    retryDelay: 250
  });
}

function removePathBestEffort(targetPath) {
  try {
    removePathWithRetry(targetPath);
  } catch (error) {
    console.warn(`Unable to remove E2E artifact ${targetPath}: ${error?.message ?? error}`);
  }
}

function createRemoteProviderCache({ id, displayName, version, source, timeoutSeconds }) {
  const providerDir = path.join(e2eProviderRoot, id);
  const manifestPath = path.join(providerDir, "provider.json");
  const sourcePath = path.join(providerDir, "provider.cjs");
  const sourceUrl = sourcePath;
  const checksum = sourceChecksum(source);
  const now = new Date().toISOString();
  const manifest = {
    schemaVersion: 1,
    id,
    displayName,
    version,
    description: `${displayName} generated by the E2E runner.`,
    runtime: "node",
    entry: "provider.cjs",
    requiredEnvVars: [],
    output: "provider-snapshot-v1",
    permissions: [],
    checksums: {
      source: checksum
    }
  };

  fs.mkdirSync(providerDir, { recursive: true });
  fs.writeFileSync(manifestPath, JSON.stringify(manifest, null, 2));
  fs.writeFileSync(sourcePath, source);
  fs.writeFileSync(
    path.join(providerDir, ".meta.json"),
    JSON.stringify(
      {
        etag: null,
        lastCheckAt: now,
        checksum,
        version,
        installedAt: now,
        updatedAt: now,
        sourceUrl,
        resolvedRuntime: process.execPath
      },
      null,
      2
    )
  );

  return {
    kind: "remote",
    id,
    name: displayName,
    enabled: true,
    version,
    manifestUrl: manifestPath,
    sourceUrl,
    providerDir,
    runtime: "node",
    resolvedRuntime: process.execPath,
    proxyUrl: null,
    autoUpdate: false,
    updateIntervalSeconds: 3600,
    timeoutSeconds,
    trustedChecksum: checksum,
    installedAt: now,
    updatedAt: now,
    lastCheckedAt: now,
    windowLabelOverrides: {},
    visibleWindowIds: [],
    envVars: {}
  };
}

function sourceChecksum(source) {
  return `sha256:${createHash("sha256").update(source).digest("hex")}`;
}

function fixtureRemoteProviderSource() {
  return `process.stderr.write(JSON.stringify({level:"info",stage:"e2e.start",message:"fixture remote provider started"}) + "\\n");
console.log(JSON.stringify({
  status: "ok",
  updatedAt: new Date().toISOString(),
  windows: [
    {
      id: "daily",
      label: "Daily",
      used: 28,
      limit: 100,
      unit: "percent",
      usedPercent: 28,
      resetAt: null,
      resetText: "resets tomorrow",
      confidence: "exact"
    },
    {
      id: "weekly",
      label: "Weekly limit",
      remaining: 64,
      limit: 100,
      unit: "percent",
      remainingPercent: 64,
      resetAt: null,
      resetText: "resets Friday",
      confidence: "estimated"
    }
  ],
  metadata: {
    fixture: true
  }
}));
`;
}

function slowRemoteProviderSource() {
  return `process.stderr.write(JSON.stringify({level:"info",stage:"e2e.slow",message:"slow remote provider started"}) + "\\n");
setTimeout(() => {
  console.log(JSON.stringify({
    status: "ok",
    updatedAt: new Date().toISOString(),
    windows: []
  }));
}, 5000);
`;
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

function assertNoConflictingAppInstance() {
  if (!isWindows) {
    return;
  }

  const command =
    "$ErrorActionPreference = 'SilentlyContinue'; " +
    "Get-Process | " +
    "Where-Object { $_.ProcessName -ieq 'QuotaBarWin' -or $_.ProcessName -ieq 'quotabarwin' } | " +
    "ForEach-Object { $_.Path }";
  const result = spawnSync("powershell.exe", ["-NoProfile", "-Command", command], {
    encoding: "utf8"
  });
  if (result.status !== 0) {
    console.warn("Unable to check for conflicting QuotaBarWin instances before E2E.");
    return;
  }

  const target = normalizePath(appExecutable);
  const conflicts = result.stdout
    .split(/\r?\n/)
    .map((line) => line.trim())
    .filter(Boolean)
    .filter((processPath) => normalizePath(processPath) !== target);

  if (conflicts.length > 0) {
    throw new Error(
      `A different QuotaBarWin instance is already running. Quit it before E2E so Tauri single-instance does not redirect startup: ${conflicts.join(", ")}`
    );
  }
}

function normalizePath(value) {
  return path.resolve(value).toLowerCase();
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

function waitForProcessExit(childProcess, timeoutMs) {
  if (childProcess.exitCode !== null || childProcess.signalCode !== null) {
    return Promise.resolve();
  }

  return new Promise((resolve) => {
    const timeout = setTimeout(resolve, timeoutMs);
    childProcess.once("exit", () => {
      clearTimeout(timeout);
      resolve();
    });
  });
}

function sleep(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}
