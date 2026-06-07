const fs = require("node:fs");

fs.writeFileSync(process.argv[2], "executed");
console.log("{}");
