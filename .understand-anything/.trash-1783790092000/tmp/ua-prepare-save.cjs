#!/usr/bin/env node
const fs = require("fs");

const [assembledPath, finalPath, scanPath, fingerprintInputPath, gitCommitHash] = process.argv.slice(2);
const assembled = JSON.parse(fs.readFileSync(assembledPath, "utf8"));
const scan = JSON.parse(fs.readFileSync(scanPath, "utf8"));
fs.writeFileSync(finalPath, JSON.stringify(assembled, null, 2) + "\n");
fs.writeFileSync(fingerprintInputPath, JSON.stringify({
  projectRoot: process.cwd(),
  sourceFilePaths: scan.files.map((file) => file.path),
  gitCommitHash,
}, null, 2) + "\n");
process.stdout.write(`saved graph=${finalPath} files=${scan.files.length}\n`);
