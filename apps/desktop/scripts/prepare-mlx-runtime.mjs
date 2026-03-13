import {
  cpSync,
  existsSync,
  globSync,
  lstatSync,
  mkdirSync,
  readFileSync,
  readdirSync,
  readlinkSync,
  realpathSync,
  rmSync,
  symlinkSync,
  unlinkSync,
  writeFileSync,
} from "node:fs";
import { execFileSync } from "node:child_process";
import path from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const desktopDir = path.resolve(scriptDir, "..");
const repoRoot = path.resolve(desktopDir, "..", "..");
const RUNTIME_FORMAT_VERSION = 2;
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

function runTool(command, args, options = {}) {
  try {
    return execFileSync(command, args, {
      encoding: "utf8",
      stdio: ["ignore", "pipe", "pipe"],
      ...options,
    });
  } catch (error) {
    const stderr = error.stderr?.toString().trim();
    const stdout = error.stdout?.toString().trim();
    const details = stderr || stdout;
    fail(details ? `${command} ${args.join(" ")} failed: ${details}` : `${command} ${args.join(" ")} failed.`);
  }
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

function readPythonVersion() {
  const runtimeVenvDir = resolveRuntimeVenvDir();
  const pyvenvCfgPath = path.join(runtimeVenvDir, "pyvenv.cfg");
  const pyvenvCfg = readFileSync(pyvenvCfgPath, "utf8");
  const versionLine = pyvenvCfg
    .split("\n")
    .map((line) => line.trim())
    .find((line) => line.startsWith("version = ") || line.startsWith("version_info = "));

  if (!versionLine) {
    fail(`Could not determine Python version from ${pyvenvCfgPath}.`);
  }

  return versionLine.split("=").at(1).trim();
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

function replaceAbsoluteSymlinkWithLocalCopy(linkPath) {
  if (!existsSync(linkPath)) {
    return;
  }

  const stats = lstatSync(linkPath);
  if (!stats.isSymbolicLink()) {
    return;
  }

  const target = readlinkSync(linkPath);
  if (!path.isAbsolute(target)) {
    return;
  }

  const resolvedTarget = realpathSync(linkPath);
  unlinkSync(linkPath);
  cpSync(resolvedTarget, linkPath);
}

function normalizePythonLaunchers(outputPythonHome) {
  const binDir = path.join(outputPythonHome, "bin");
  const python312 = path.join(binDir, "python3.12");
  replaceAbsoluteSymlinkWithLocalCopy(python312);

  for (const launcherName of ["python", "python3"]) {
    const launcherPath = path.join(binDir, launcherName);
    if (!existsSync(launcherPath)) {
      continue;
    }

    try {
      unlinkSync(launcherPath);
    } catch (error) {
      fail(`Unable to replace ${launcherPath} with a relative symlink: ${error}`);
    }

    symlinkSync("python3.12", launcherPath);
  }
}

function walkFiles(rootDir) {
  const files = [];
  for (const entry of readdirSync(rootDir, { withFileTypes: true })) {
    const entryPath = path.join(rootDir, entry.name);
    if (entry.isDirectory()) {
      files.push(...walkFiles(entryPath));
      continue;
    }
    if (entry.isFile()) {
      files.push(entryPath);
    }
  }
  return files;
}

function listMachODeps(filePath) {
  try {
    const output = execFileSync("otool", ["-L", filePath], {
      encoding: "utf8",
      stdio: ["ignore", "pipe", "pipe"],
    });
    return output
      .split("\n")
      .slice(1)
      .map((line) => line.trim())
      .filter(Boolean)
      .map((line) => line.split(" (compatibility version ")[0]);
  } catch {
    return null;
  }
}

function relocateBundledPython(outputDir, pythonVersion) {
  const outputPythonHome = path.join(outputDir, "python-home");
  normalizePythonLaunchers(outputPythonHome);

  const majorMinor = pythonVersion.split(".").slice(0, 2).join(".");
  const bundledLibpython = path.join(outputPythonHome, "lib", `libpython${majorMinor}.dylib`);
  if (!existsSync(bundledLibpython)) {
    fail(`Bundled Python library not found at ${bundledLibpython}.`);
  }

  runTool("install_name_tool", ["-id", `@rpath/libpython${majorMinor}.dylib`, bundledLibpython]);

  for (const filePath of walkFiles(outputDir)) {
    const deps = listMachODeps(filePath);
    if (!deps) {
      continue;
    }

    const loaderToken = filePath.includes(`${path.sep}bin${path.sep}`) ? "@executable_path" : "@loader_path";
    const relativeLibpythonPath = path.relative(path.dirname(filePath), bundledLibpython);
    const relocatedLibpythonPath = `${loaderToken}/${relativeLibpythonPath}`;

    for (const dependency of deps) {
      if (
        dependency.includes("Python.framework/Versions/")
        || dependency.endsWith(`/libpython${majorMinor}.dylib`)
      ) {
        runTool("install_name_tool", ["-change", dependency, relocatedLibpythonPath, filePath]);
      }
    }
  }
}

function main() {
  const pythonHome = readPythonHome();
  const pythonVersion = readPythonVersion();
  const pythonLibDir = findPythonLibDir();
  const sitePackagesDir = path.join(pythonLibDir, "site-packages");
  const pythonLibVersion = path.basename(pythonLibDir);

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
  relocateBundledPython(outputDir, pythonVersion);

  const bundledSitePackagesDir = path.join(outputDir, "site-packages");
  mkdirSync(bundledSitePackagesDir, { recursive: true });
  copySelectedSitePackages(sitePackagesDir, bundledSitePackagesDir);

  writeFileSync(
    path.join(outputDir, "manifest.json"),
    JSON.stringify(
      {
        formatVersion: RUNTIME_FORMAT_VERSION,
        pythonHome: path.basename(pythonHome),
        pythonVersion: pythonLibVersion,
        requiredSitePackages: REQUIRED_SITE_PACKAGE_PATTERNS,
      },
      null,
      2,
    ),
  );

  console.log(`Prepared MLX runtime at ${outputDir}`);
}

main();
