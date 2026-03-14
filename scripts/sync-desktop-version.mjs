import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");

function fail(message) {
  console.error(message);
  process.exit(1);
}

function normalizeVersion(input) {
  const raw = input.trim().replace(/^v/, "");
  const match = raw.match(
    /^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)(?:-[0-9A-Za-z.-]+)?(?:\+[0-9A-Za-z.-]+)?$/,
  );

  if (!match) {
    fail(`Invalid version "${input}". Use semver like v0.3.0 or 0.3.0.`);
  }

  return raw;
}

function writeJsonVersion(filePath, version) {
  const content = JSON.parse(fs.readFileSync(filePath, "utf8"));
  content.version = version;
  fs.writeFileSync(filePath, `${JSON.stringify(content, null, 2)}\n`);
}

const rawVersion = process.argv[2];

if (!rawVersion) {
  fail("Usage: node ./scripts/sync-desktop-version.mjs v0.3.0");
}

const version = normalizeVersion(rawVersion);
const jsonFiles = [
  path.join(repoRoot, "apps", "desktop", "package.json"),
  path.join(repoRoot, "apps", "desktop", "src-tauri", "tauri.conf.json"),
];

for (const filePath of jsonFiles) {
  writeJsonVersion(filePath, version);
}

const cargoTomlPath = path.join(repoRoot, "apps", "desktop", "src-tauri", "Cargo.toml");
const cargoToml = fs.readFileSync(cargoTomlPath, "utf8");
const versionPattern = /^version = ".*"$/m;

if (!versionPattern.test(cargoToml)) {
  fail("Could not find a version entry in apps/desktop/src-tauri/Cargo.toml.");
}

fs.writeFileSync(cargoTomlPath, cargoToml.replace(versionPattern, `version = "${version}"`));

console.log(`Synced desktop version to ${version}`);
