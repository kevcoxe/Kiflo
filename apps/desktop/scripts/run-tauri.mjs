import { spawn } from "node:child_process";
import path from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const tauriBinary = path.resolve(
  scriptDir,
  "..",
  "..",
  "..",
  "node_modules",
  ".bin",
  process.platform === "win32" ? "tauri.cmd" : "tauri",
);

const args = process.argv.slice(2);
const env = { ...process.env };

if (process.platform === "darwin") {
  const availabilityFlag = "-D_LIBCPP_DISABLE_AVAILABILITY";
  env.CMAKE_CXX_FLAGS = [env.CMAKE_CXX_FLAGS, availabilityFlag].filter(Boolean).join(" ");
  env.CMAKE_C_FLAGS = [env.CMAKE_C_FLAGS, availabilityFlag].filter(Boolean).join(" ");
  env.MACOSX_DEPLOYMENT_TARGET = env.MACOSX_DEPLOYMENT_TARGET || "15.0";
  env.CMAKE_OSX_DEPLOYMENT_TARGET =
    env.CMAKE_OSX_DEPLOYMENT_TARGET || env.MACOSX_DEPLOYMENT_TARGET;
  env.GGML_NATIVE = env.GGML_NATIVE || "OFF";
}

const child = spawn(tauriBinary, args, {
  env,
  stdio: "inherit",
});

child.on("exit", (code, signal) => {
  if (signal) {
    process.kill(process.pid, signal);
    return;
  }

  process.exit(code ?? 1);
});

child.on("error", (error) => {
  console.error(`Unable to launch the Tauri CLI at ${tauriBinary}: ${error.message}`);
  process.exit(1);
});
