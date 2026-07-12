#!/usr/bin/env node
const fs = require("fs");

const [metaPath, gitCommitHash, analyzedFiles] = process.argv.slice(2);
fs.writeFileSync(metaPath, JSON.stringify({
  lastAnalyzedAt: new Date().toISOString(),
  gitCommitHash,
  version: "1.0.0",
  analyzedFiles: Number(analyzedFiles),
}, null, 2) + "\n");
