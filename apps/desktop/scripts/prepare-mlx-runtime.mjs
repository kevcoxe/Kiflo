import { cpSync, existsSync, globSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const desktopDir = path.resolve(scriptDir, "..");
const repoRoot = path.resolve(desktopDir, "..", "..");
const runtimeVenvCandidates = [
  path.join(repoRoot, "apps", "mlx-runtime", ".venv"),
  path.join(repoRoot, "apps", "api", ".venv"),
];
const outputDir = path.join(desktopDir, "src-tauri", "target", "bundled-runtime");

const REQUIRED_SITE_PACKAGE_PATTERNS = [
  "81d243bd2c585b0f4821__mypyc.cpython-*.so",
  "anyio",
  "anyio-*.dist-info",
  "PyYAML-*.dist-info",
  "pyyaml-*.dist-info",
  "certifi",
  "certifi-*.dist-info",
  "charset_normalizer",
  "charset_normalizer-*.dist-info",
  "filelock",
  "filelock-*.dist-info",
  "fsspec",
  "fsspec-*.dist-info",
  "hf_xet",
  "hf_xet-*.dist-info",
  "h11",
  "h11-*.dist-info",
  "huggingface_hub",
  "huggingface_hub-*.dist-info",
  "httpcore",
  "httpcore-*.dist-info",
  "httpx",
  "httpx-*.dist-info",
  "idna",
  "idna-*.dist-info",
  "llvmlite",
  "llvmlite-*.dist-info",
  "mlx",
  "mlx-*.dist-info",
  "mlx_metal-*.dist-info",
  "mlx_whisper",
  "mlx_whisper-*.dist-info",
  "more_itertools",
  "more_itertools-*.dist-info",
  "numpy",
  "numpy-*.dist-info",
  "numba",
  "numba-*.dist-info",
  "packaging",
  "packaging-*.dist-info",
  "regex",
  "regex-*.dist-info",
  "requests",
  "requests-*.dist-info",
  "scipy",
  "scipy-*.dist-info",
  "tiktoken",
  "tiktoken-*.dist-info",
  "tiktoken_ext",
  "tqdm",
  "tqdm-*.dist-info",
  "typing_extensions-*.dist-info",
  "typing_extensions.py",
  "urllib3",
  "urllib3-*.dist-info",
  "yaml",
];

function fail(message) {
  console.error(message);
  process.exit(1);
}

function resolveRuntimeVenvDir() {
  return runtimeVenvCandidates.find((candidate) => existsSync(path.join(candidate, "pyvenv.cfg")))
    ?? runtimeVenvCandidates[0];
}

function readPythonHome() {
  const runtimeVenvDir = resolveRuntimeVenvDir();
  const pyvenvCfgPath = path.join(runtimeVenvDir, "pyvenv.cfg");
  if (!existsSync(pyvenvCfgPath)) {
    fail(
      "Missing apps/mlx-runtime/.venv. Run `uv sync --project apps/mlx-runtime --extra dev` before building the desktop app.",
    );
  }

  const pyvenvCfg = readFileSync(pyvenvCfgPath, "utf8");
  const homeLine = pyvenvCfg
    .split("\n")
    .map((line) => line.trim())
    .find((line) => line.startsWith("home = "));
  if (!homeLine) {
    fail(`Could not determine Python home from ${pyvenvCfgPath}.`);
  }

  return path.dirname(homeLine.slice("home = ".length));
}

function findPythonLibDir() {
  const runtimeVenvDir = resolveRuntimeVenvDir();
  const matches = globSync("python*", {
    cwd: path.join(runtimeVenvDir, "lib"),
    onlyDirectories: true,
  });
  if (matches.length !== 1) {
    fail(
      `Expected exactly one Python lib directory under ${path.relative(repoRoot, path.join(runtimeVenvDir, "lib"))}.`,
    );
  }
  return path.join(runtimeVenvDir, "lib", matches[0]);
}

function copySelectedSitePackages(sitePackagesDir, destinationDir) {
  const copied = new Set();
  for (const pattern of REQUIRED_SITE_PACKAGE_PATTERNS) {
    const matches = globSync(pattern, {
      cwd: sitePackagesDir,
      dot: true,
    });
    if (matches.length === 0) {
      fail(
        `Missing required site-package asset matching \`${pattern}\` in ${sitePackagesDir}. Run \`uv sync --project apps/mlx-runtime --extra dev\` to refresh the helper environment.`,
      );
    }

    for (const relativePath of matches) {
      if (copied.has(relativePath)) {
        continue;
      }
      copied.add(relativePath);
      cpSync(path.join(sitePackagesDir, relativePath), path.join(destinationDir, relativePath), {
        recursive: true,
      });
    }
  }
}

function main() {
  const pythonHome = readPythonHome();
  const pythonLibDir = findPythonLibDir();
  const sitePackagesDir = path.join(pythonLibDir, "site-packages");
  const pythonVersion = path.basename(pythonLibDir);

  rmSync(outputDir, {
    force: true,
    recursive: true,
    maxRetries: 5,
    retryDelay: 100,
  });
  mkdirSync(outputDir, { recursive: true });

  cpSync(pythonHome, path.join(outputDir, "python-home"), {
    recursive: true,
  });

  const bundledSitePackagesDir = path.join(outputDir, "site-packages");
  mkdirSync(bundledSitePackagesDir, { recursive: true });
  copySelectedSitePackages(sitePackagesDir, bundledSitePackagesDir);

  writeFileSync(
    path.join(outputDir, "manifest.json"),
    JSON.stringify(
      {
        pythonHome: path.basename(pythonHome),
        pythonVersion,
        requiredSitePackages: REQUIRED_SITE_PACKAGE_PATTERNS,
      },
      null,
      2,
    ),
  );

  console.log(`Prepared MLX runtime at ${outputDir}`);
}

main();
