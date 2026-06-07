const fs = require("node:fs");
const path = require("node:path");

const file = path.join(__dirname, "..", "..", "docs", "specs", "fixtures", "provider_outputs", "bigmodel_quota_limit.json");
process.stdout.write(fs.readFileSync(file, "utf8"));
