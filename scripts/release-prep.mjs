import { execFileSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const changelogPath = process.env.CHANGELOG_PATH
  ? path.resolve(process.cwd(), process.env.CHANGELOG_PATH)
  : path.join(repoRoot, "CHANGELOG.md");

function fail(message) {
  console.error(message);
  process.exit(1);
}

function normalizeVersion(input) {
  const raw = input.trim().replace(/^v/, "");
  const match = raw.match(/^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)$/);

  if (!match) {
    fail(`Invalid version "${input}". Use semver like v0.1.4 or 0.1.4.`);
  }

  return raw;
}

function compareVersions(left, right) {
  const leftParts = left.split(".").map(Number);
  const rightParts = right.split(".").map(Number);

  for (let index = 0; index < leftParts.length; index += 1) {
    if (leftParts[index] !== rightParts[index]) {
      return leftParts[index] - rightParts[index];
    }
  }

  return 0;
}

function runGit(args) {
  return execFileSync("git", args, {
    cwd: repoRoot,
    encoding: "utf8",
  }).trim();
}

function escapeRegExp(value) {
  return value.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}

function readValidTags() {
  const output = runGit(["tag", "--merged", "HEAD", "--list", "v*"]);

  return output
    .split(/\r?\n/)
    .map((tag) => tag.trim())
    .filter(Boolean)
    .map((tag) => ({ tag, version: tag.replace(/^v/, "") }))
    .filter(({ version }) => /^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)$/.test(version))
    .sort((left, right) => compareVersions(left.version, right.version));
}

function readCommitSubjects(previousTag) {
  const args = ["log", "--reverse", "--no-merges", "--pretty=format:%s"];

  if (previousTag) {
    args.push(`${previousTag}..HEAD`);
  }

  const output = runGit(args);

  return output
    .split(/\r?\n/)
    .map((line) => line.trim())
    .filter(Boolean);
}

const rawVersion = process.argv[2];

if (!rawVersion) {
  fail("Usage: npm run release:prep -- v0.1.4");
}

if (!fs.existsSync(changelogPath)) {
  fail(`Missing changelog file at ${changelogPath}`);
}

const version = normalizeVersion(rawVersion);
const tagName = `v${version}`;
const changelog = fs.readFileSync(changelogPath, "utf8");
const sectionPattern = new RegExp(`^## \\[${escapeRegExp(version)}\\] - `, "m");

if (sectionPattern.test(changelog)) {
  fail(`CHANGELOG already contains an entry for ${tagName}.`);
}

const validTags = readValidTags();

if (validTags.some(({ tag }) => tag === tagName)) {
  fail(`Git tag ${tagName} already exists.`);
}

const previousTag = validTags.filter(({ version: existing }) => compareVersions(existing, version) < 0).at(-1)?.tag;
const commits = readCommitSubjects(previousTag);

if (commits.length === 0) {
  fail(`No non-merge commits found since ${previousTag ?? "the repository start"}.`);
}

const firstSectionIndex = changelog.search(/^## \[/m);

if (firstSectionIndex === -1) {
  fail(`Could not find a release section in ${changelogPath}.`);
}

const today = new Date().toISOString().slice(0, 10);
const nextSection = [
  `## [${version}] - ${today}`,
  "",
  "### Changed",
  ...commits.map((subject) => `- ${subject}`),
  "",
  "",
].join("\n");

const updatedChangelog = `${changelog.slice(0, firstSectionIndex)}${nextSection}${changelog.slice(firstSectionIndex)}`;
fs.writeFileSync(changelogPath, updatedChangelog);

console.log(`Added ${tagName} release notes to ${changelogPath}`);
