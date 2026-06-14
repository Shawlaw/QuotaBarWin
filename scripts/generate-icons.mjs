import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { existsSync, mkdtempSync, readdirSync, readFileSync, rmSync, statSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = path.resolve(fileURLToPath(new URL("..", import.meta.url)));
const defaultSource = path.join(repoRoot, "src-tauri", "icons", "source.svg");
const defaultOutput = path.join(repoRoot, "src-tauri", "icons");
const generatedExtensions = new Set([".icns", ".ico", ".png", ".xml"]);
const contentCheckedExtensions = new Set([".ico", ".png", ".xml"]);

const args = process.argv.slice(2);
const check = args.includes("--check");
const sourceArg = args.find((arg) => !arg.startsWith("--"));
const source = path.resolve(repoRoot, sourceArg ?? defaultSource);
const output = defaultOutput;

if (!existsSync(source)) {
  throw new Error(`Icon source not found: ${source}`);
}

function tauriBin() {
  const localBin = path.join(repoRoot, "node_modules", "@tauri-apps", "cli", "tauri.js");

  if (!existsSync(localBin)) {
    throw new Error("Tauri CLI not found. Run `npm install` before generating icons.");
  }

  return localBin;
}

function runTauriIcon(targetDir) {
  const result = spawnSync(process.execPath, [tauriBin(), "icon", "--output", targetDir, source], {
    cwd: repoRoot,
    stdio: "inherit",
  });

  if (result.error) {
    throw result.error;
  }

  if (result.status !== 0) {
    throw new Error(`Icon generation failed with exit code ${result.status ?? "unknown"}.`);
  }
}

function listGeneratedFiles(dir, base = dir) {
  return readdirSync(dir).flatMap((entry) => {
    const fullPath = path.join(dir, entry);
    const relativePath = path.relative(base, fullPath);

    if (statSync(fullPath).isDirectory()) {
      return listGeneratedFiles(fullPath, base);
    }

    return generatedExtensions.has(path.extname(entry).toLowerCase()) ? [relativePath] : [];
  });
}

function hashFile(filePath) {
  return createHash("sha256").update(readFileSync(filePath)).digest("hex");
}

function checkIcons() {
  const tempDir = mkdtempSync(path.join(tmpdir(), "quotabarwin-icons-"));

  try {
    runTauriIcon(tempDir);

    const expected = listGeneratedFiles(tempDir).sort();
    const actual = listGeneratedFiles(output).sort();
    const missing = expected.filter((file) => !actual.includes(file));
    const extra = actual.filter((file) => !expected.includes(file));
    const changed = expected.filter((file) => {
      if (!contentCheckedExtensions.has(path.extname(file).toLowerCase())) {
        return false;
      }

      const expectedPath = path.join(tempDir, file);
      const actualPath = path.join(output, file);
      return existsSync(actualPath) && hashFile(expectedPath) !== hashFile(actualPath);
    });

    if (missing.length || extra.length || changed.length) {
      const detail = [
        missing.length ? `Missing: ${missing.join(", ")}` : "",
        extra.length ? `Extra: ${extra.join(", ")}` : "",
        changed.length ? `Changed: ${changed.join(", ")}` : "",
      ]
        .filter(Boolean)
        .join("\n");

      throw new Error(`Generated icons are out of date.\n${detail}\nRun \`npm run icons:generate\`.`);
    }

    console.log(`Icon outputs match ${path.relative(repoRoot, source)}.`);
  } finally {
    rmSync(tempDir, { force: true, recursive: true });
  }
}

if (check) {
  checkIcons();
} else {
  runTauriIcon(output);
  console.log(`Generated icons in ${path.relative(repoRoot, output)} from ${path.relative(repoRoot, source)}.`);
}
