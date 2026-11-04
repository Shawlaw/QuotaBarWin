import { createHash } from "node:crypto";
import { readFile, writeFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

// Generates the provider bundle that QuotaBarWin embeds into the executable.
// The bundle is deterministic: it contains no timestamps, so `--check` can fail
// a build when examples/remote-providers changed without regenerating it.

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(scriptDir, "..");
const examplesDir = path.join(repoRoot, "examples", "remote-providers");
const defaultOutput = path.join(repoRoot, "src-tauri", "src", "provider_bundle.json");
const defaultBaseUrl =
  "https://raw.githubusercontent.com/Shawlaw/QuotaBarWin/main/examples/remote-providers/";

function parseArgs(argv) {
  const options = { check: false, baseUrl: defaultBaseUrl, out: defaultOutput };
  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];
    if (arg === "--check") {
      options.check = true;
    } else if (arg === "--base-url") {
      options.baseUrl = argv[index + 1];
      index += 1;
    } else if (arg.startsWith("--base-url=")) {
      options.baseUrl = arg.slice("--base-url=".length);
    } else if (arg === "--out") {
      options.out = path.resolve(argv[index + 1]);
      index += 1;
    } else if (arg.startsWith("--out=")) {
      options.out = path.resolve(arg.slice("--out=".length));
    } else {
      throw new Error(`Unknown argument: ${arg}`);
    }
  }
  if (!options.baseUrl || !/^https?:\/\//.test(options.baseUrl)) {
    throw new Error("--base-url must be an absolute http(s) URL");
  }
  if (!options.baseUrl.endsWith("/")) {
    options.baseUrl += "/";
  }
  return options;
}

function sha256(text) {
  return `sha256:${createHash("sha256").update(text, "utf8").digest("hex")}`;
}

function parseVersion(version) {
  const core = String(version).trim().split(/[-+]/, 1)[0];
  const parts = core.split(".").map((part) => Number(part));
  return parts.length > 0 && parts.every((part) => Number.isInteger(part) && part >= 0)
    ? parts
    : null;
}

function isVersionAtMost(version, maximum) {
  const left = parseVersion(version);
  const right = parseVersion(maximum);
  if (!left || !right) {
    return false;
  }
  const length = Math.max(left.length, right.length);
  for (let index = 0; index < length; index += 1) {
    const a = left[index] ?? 0;
    const b = right[index] ?? 0;
    if (a !== b) {
      return a < b;
    }
  }
  return true;
}

function resolveRelative(baseUrl, relative) {
  const trimmed = String(relative).trim();
  if (/^[a-z][a-z0-9+.-]*:/i.test(trimmed) || trimmed.startsWith("/") || trimmed.includes("..")) {
    throw new Error(`Unsupported bundle path '${relative}'; use a relative path without '..'`);
  }
  return new URL(trimmed, baseUrl).toString();
}

async function buildBundle(options) {
  const packageJson = JSON.parse(await readFile(path.join(repoRoot, "package.json"), "utf8"));
  const registryText = await readFile(path.join(examplesDir, "registry.json"), "utf8");
  const registry = JSON.parse(registryText);
  if (registry.schemaVersion !== 1) {
    throw new Error(`Unsupported registry schemaVersion ${registry.schemaVersion}`);
  }
  if (!Array.isArray(registry.providers) || registry.providers.length === 0) {
    throw new Error("Registry contains no providers");
  }

  const seen = new Set();
  const providers = [];
  for (const entry of registry.providers) {
    const id = String(entry.id ?? "").trim();
    if (!id) {
      throw new Error("Registry entry is missing id");
    }
    if (seen.has(id)) {
      throw new Error(`Duplicate provider id '${id}' in registry`);
    }
    seen.add(id);

    const manifestUrl = resolveRelative(options.baseUrl, entry.providerUrl);
    const manifestPath = path.join(examplesDir, ...String(entry.providerUrl).split("/"));
    const manifestText = await readFile(manifestPath, "utf8");
    const manifestChecksum = sha256(manifestText);
    if (entry.checksum && entry.checksum !== manifestChecksum) {
      throw new Error(
        `${id} manifest checksum mismatch: registry has ${entry.checksum}, actual ${manifestChecksum}`,
      );
    }

    let manifest;
    try {
      manifest = JSON.parse(manifestText);
    } catch (error) {
      throw new Error(`${id} manifest is not valid JSON: ${error.message}`);
    }
    if (manifest.id !== id) {
      throw new Error(`${id} manifest id is '${manifest.id}'`);
    }
    if (manifest.schemaVersion !== 2) {
      throw new Error(`${id} must use schemaVersion 2`);
    }
    if (manifest.runtime !== "builtin-js") {
      throw new Error(`${id} must use the builtin-js runtime to be bundled`);
    }
    if (!manifest.minAppVersion) {
      throw new Error(`${id} must declare minAppVersion`);
    }
    if (!isVersionAtMost(manifest.minAppVersion, packageJson.version)) {
      throw new Error(
        `${id} requires app ${manifest.minAppVersion}, current app is ${packageJson.version}`,
      );
    }
    if (manifest.output !== "provider-snapshot-v1") {
      throw new Error(`${id} output must be provider-snapshot-v1`);
    }

    const sourceUrl = resolveRelative(manifestUrl, manifest.entry);
    const sourcePath = path.join(path.dirname(manifestPath), ...String(manifest.entry).split("/"));
    const sourceText = await readFile(sourcePath, "utf8");
    const sourceChecksum = sha256(sourceText);
    const declaredSourceChecksum = manifest.checksums?.source;
    if (!declaredSourceChecksum) {
      throw new Error(`${id} manifest is missing checksums.source`);
    }
    if (declaredSourceChecksum !== sourceChecksum) {
      throw new Error(
        `${id} source checksum mismatch: manifest has ${declaredSourceChecksum}, actual ${sourceChecksum}`,
      );
    }

    providers.push({
      id,
      displayName: manifest.displayName ?? id,
      version: manifest.version ?? null,
      manifestUrl,
      sourceUrl,
      manifestChecksum,
      sourceChecksum,
      manifest: manifestText,
      source: sourceText,
    });
  }

  return {
    schemaVersion: 1,
    appVersion: packageJson.version,
    registryUrl: resolveRelative(options.baseUrl, "registry.json"),
    providers,
  };
}

async function main() {
  const options = parseArgs(process.argv.slice(2));
  const bundle = await buildBundle(options);
  const serialized = `${JSON.stringify(bundle, null, 2)}\n`;

  if (options.check) {
    let existing = null;
    try {
      existing = await readFile(options.out, "utf8");
    } catch {
      // Missing bundle is a check failure below.
    }
    if (existing !== serialized) {
      console.error(
        `provider bundle is out of date: ${path.relative(repoRoot, options.out)}\n` +
          "Run `npm run providers:bundle` and commit the result.",
      );
      process.exitCode = 1;
      return;
    }
    console.log(
      `provider bundle is current (${bundle.providers.length} providers, app ${bundle.appVersion})`,
    );
    return;
  }

  await writeFile(options.out, serialized, "utf8");
  console.log(
    `wrote ${path.relative(repoRoot, options.out)} (${bundle.providers.length} providers, app ${bundle.appVersion})`,
  );
}

main().catch((error) => {
  console.error(error instanceof Error ? error.message : String(error));
  process.exitCode = 1;
});
