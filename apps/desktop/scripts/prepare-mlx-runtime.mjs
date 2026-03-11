import { cpSync, existsSync, globSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const desktopDir = path.resolve(scriptDir, "..");
const repoRoot = path.resolve(desktopDir, "..", "..");
const apiVenvDir = path.join(repoRoot, "apps", "api", ".venv");
const pyvenvCfgPath = path.join(apiVenvDir, "pyvenv.cfg");
const outputDir = path.join(desktopDir, "src-tauri", "target", "bundled-runtime");

const REQUIRED_SITE_PACKAGE_PATTERNS = [
  "81d243bd2c585b0f4821__mypyc.cpython-*.so",
  "PyYAML-*.dist-info",
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
  "huggingface_hub",
  "huggingface_hub-*.dist-info",
  "idna",
  "idna-*.dist-info",
  "mlx",
  "mlx-*.dist-info",
  "mlx_metal-*.dist-info",
  "mlx_whisper",
  "mlx_whisper-*.dist-info",
  "more_itertools",
  "more_itertools-*.dist-info",
  "numpy",
  "numpy-*.dist-info",
  "packaging",
  "packaging-*.dist-info",
  "regex",
  "regex-*.dist-info",
  "requests",
  "requests-*.dist-info",
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

function readPythonHome() {
  if (!existsSync(pyvenvCfgPath)) {
    fail(
      "Missing apps/api/.venv. Run `uv sync --project apps/api --extra dev` before building the desktop app.",
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
  const matches = globSync("python*", {
    cwd: path.join(apiVenvDir, "lib"),
    onlyDirectories: true,
  });
  if (matches.length !== 1) {
    fail("Expected exactly one Python lib directory under apps/api/.venv/lib.");
  }
  return path.join(apiVenvDir, "lib", matches[0]);
}

function copySelectedSitePackages(sitePackagesDir, destinationDir) {
  const copied = new Set();
  for (const pattern of REQUIRED_SITE_PACKAGE_PATTERNS) {
    const matches = globSync(pattern, {
      cwd: sitePackagesDir,
      dot: true,
    });
    if (matches.length === 0) {
      fail(`Missing required site-package asset matching \`${pattern}\`.`);
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

  rmSync(outputDir, { force: true, recursive: true });
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
