import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const file = path.join(
  __dirname,
  "..",
  "..",
  "docs",
  "specs",
  "fixtures",
  "provider_outputs",
  "kimi_coding_usage.json"
);
process.stdout.write(fs.readFileSync(file, "utf8"));
