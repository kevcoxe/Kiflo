import { createHash } from "node:crypto";
import {
  createReadStream,
  createWriteStream,
  existsSync,
  mkdirSync,
  readdirSync,
  readFileSync,
  renameSync,
  rmSync,
  statSync,
  writeFileSync,
} from "node:fs";
import path from "node:path";
import { Readable } from "node:stream";
import { pipeline } from "node:stream/promises";
import { fileURLToPath } from "node:url";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const bundledModelDir = path.resolve(scriptDir, "..", "src-tauri", "target", "bundled-whisper-model");
const catalogPath = path.resolve(scriptDir, "..", "src-tauri", "resources", "whisper-models.json");
const modelCatalog = JSON.parse(readFileSync(catalogPath, "utf8"));
const bundledModels = modelCatalog.filter((entry) => entry.bundled);

if (bundledModels.length === 0) {
  throw new Error(`No bundled whisper.cpp models were defined in ${catalogPath}.`);
}

function sha1ForFile(filePath) {
  return new Promise((resolve, reject) => {
    const hash = createHash("sha1");
    const stream = createReadStream(filePath);
    stream.on("error", reject);
    stream.on("data", (chunk) => hash.update(chunk));
    stream.on("end", () => resolve(hash.digest("hex")));
  });
}

async function verifyExistingModel(modelPath, expectedSha1) {
  if (!existsSync(modelPath)) {
    return false;
  }

  const digest = await sha1ForFile(modelPath);
  if (digest !== expectedSha1) {
    rmSync(modelPath, { force: true });
    return false;
  }

  return true;
}

async function downloadModel(modelPath, modelUrl, modelFileName, modelSha1) {
  const tempPath = `${modelPath}.download`;
  rmSync(tempPath, { force: true });

  const response = await fetch(modelUrl);
  if (!response.ok || !response.body) {
    throw new Error(`Failed to download ${modelFileName}: ${response.status} ${response.statusText}`);
  }

  await pipeline(Readable.fromWeb(response.body), createWriteStream(tempPath));

  const digest = await sha1ForFile(tempPath);
  if (digest !== modelSha1) {
    rmSync(tempPath, { force: true });
    throw new Error(
      `Downloaded ${modelFileName} but the SHA1 did not match. Expected ${modelSha1}, received ${digest}.`,
    );
  }

  renameSync(tempPath, modelPath);
}

function removeStaleModels() {
  const bundledFileNames = new Set(bundledModels.map((entry) => entry.fileName));
  for (const entry of readdirSync(bundledModelDir, { withFileTypes: true })) {
    if (!entry.isFile()) {
      continue;
    }

    if (
      !entry.name.startsWith("ggml-") ||
      !entry.name.endsWith(".bin") ||
      bundledFileNames.has(entry.name)
    ) {
      continue;
    }

    rmSync(path.join(bundledModelDir, entry.name), { force: true });
  }
}

async function main() {
  mkdirSync(bundledModelDir, { recursive: true });
  const manifestPath = path.join(bundledModelDir, "manifest.json");
  removeStaleModels();

  const manifest = [];
  for (const model of bundledModels) {
    const modelPath = path.join(bundledModelDir, model.fileName);

    if (!(await verifyExistingModel(modelPath, model.sha1))) {
      console.log(`Downloading bundled whisper.cpp model to ${modelPath}`);
      await downloadModel(modelPath, model.url, model.fileName, model.sha1);
    }

    manifest.push({
      id: model.id,
      fileName: model.fileName,
      sha1: model.sha1,
      sizeBytes: statSync(modelPath).size,
      source: model.url,
    });
  }

  writeFileSync(manifestPath, JSON.stringify({ models: manifest }, null, 2));

  console.log(`Prepared bundled whisper.cpp assets in ${bundledModelDir}`);
}

main().catch((error) => {
  console.error(error instanceof Error ? error.message : String(error));
  process.exit(1);
});
