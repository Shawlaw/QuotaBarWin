const fs = require("node:fs");
const path = require("node:path");

const file = path.join(__dirname, "..", "..", "docs", "specs", "fixtures", "provider_outputs", "kimi_coding_usage.json");
process.stdout.write(fs.readFileSync(file, "utf8"));
