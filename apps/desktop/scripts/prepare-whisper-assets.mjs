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
const bundledModel = modelCatalog.find((entry) => entry.bundled);

if (!bundledModel) {
  throw new Error(`No bundled whisper.cpp model was defined in ${catalogPath}.`);
}

const { id: modelId, fileName: modelFileName, url: modelUrl, sha1: modelSha1 } = bundledModel;

function sha1ForFile(filePath) {
  return new Promise((resolve, reject) => {
    const hash = createHash("sha1");
    const stream = createReadStream(filePath);
    stream.on("error", reject);
    stream.on("data", (chunk) => hash.update(chunk));
    stream.on("end", () => resolve(hash.digest("hex")));
  });
}

async function verifyExistingModel(modelPath) {
  if (!existsSync(modelPath)) {
    return false;
  }

  const digest = await sha1ForFile(modelPath);
  if (digest !== modelSha1) {
    rmSync(modelPath, { force: true });
    return false;
  }

  return true;
}

async function downloadModel(modelPath) {
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
  for (const entry of readdirSync(bundledModelDir, { withFileTypes: true })) {
    if (!entry.isFile()) {
      continue;
    }

    if (!entry.name.startsWith("ggml-") || !entry.name.endsWith(".bin") || entry.name === modelFileName) {
      continue;
    }

    rmSync(path.join(bundledModelDir, entry.name), { force: true });
  }
}

async function main() {
  mkdirSync(bundledModelDir, { recursive: true });

  const modelPath = path.join(bundledModelDir, modelFileName);
  const manifestPath = path.join(bundledModelDir, "manifest.json");
  removeStaleModels();

  if (!(await verifyExistingModel(modelPath))) {
    console.log(`Downloading bundled whisper.cpp model to ${modelPath}`);
    await downloadModel(modelPath);
  }

  writeFileSync(
    manifestPath,
    JSON.stringify(
      {
        id: modelId,
        fileName: modelFileName,
        sha1: modelSha1,
        sizeBytes: statSync(modelPath).size,
        source: modelUrl,
      },
      null,
      2,
    ),
  );

  console.log(`Prepared bundled whisper.cpp assets in ${bundledModelDir}`);
}

main().catch((error) => {
  console.error(error instanceof Error ? error.message : String(error));
  process.exit(1);
});
