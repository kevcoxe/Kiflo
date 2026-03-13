import { show } from "@tauri-apps/api/app";
import { invoke, isTauri } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import {
  register,
  unregister,
  unregisterAll,
} from "@tauri-apps/plugin-global-shortcut";
import type {
  ClipboardSnapshot,
  CleanupResponse,
  DictationMode,
  HistoryMetrics,
  HistoryRecord,
  ModelStatus,
  NewHistoryRecord,
  PermissionStatus,
  RecordedAudio,
  Settings,
  TranscriptionResponse,
} from "@tator/shared-types";

function formatDesktopError(error: unknown): string {
  if (error instanceof Error && error.message) {
    return error.message;
  }

  if (typeof error === "string" && error.length > 0) {
    return error;
  }

  if (typeof error === "object" && error !== null) {
    try {
      return JSON.stringify(error);
    } catch {
      return "Unknown desktop bridge error.";
    }
  }

  return "Unknown desktop bridge error.";
}

export type HudPhase = "hidden" | "recording" | "processing" | "error";

export interface HudState {
  visible: boolean;
  phase: HudPhase;
}

export interface HudMeterPayload {
  level: number;
}

async function invokeDesktop<T>(command: string, args?: Record<string, unknown>) {
  if (!isTauri()) {
    throw new Error(
      "This UI is not running inside Tauri. Start it with `npm run tauri:dev`, not only the Vite dev server.",
    );
  }

  try {
    return await invoke<T>(command, args);
  } catch (error) {
    throw new Error(formatDesktopError(error));
  }
}

export async function getSettings(): Promise<Settings> {
  return invokeDesktop<Settings>("get_settings");
}

export async function updateSettings(settings: Settings): Promise<Settings> {
  return invokeDesktop<Settings>("update_settings", { settings });
}

export async function listHistory(limit = 10): Promise<HistoryRecord[]> {
  return invokeDesktop<HistoryRecord[]>("list_history", { limit });
}

export async function createHistoryRecord(
  record: NewHistoryRecord,
): Promise<HistoryRecord> {
  return invokeDesktop<HistoryRecord>("create_history_record", { record });
}

export async function getHistoryMetrics(): Promise<HistoryMetrics> {
  return invokeDesktop<HistoryMetrics>("get_history_metrics");
}

export async function getPermissionStatus(): Promise<PermissionStatus> {
  return invokeDesktop<PermissionStatus>("get_permission_status");
}

export async function startRecording(
  pauseBackgroundAudio: boolean,
): Promise<void> {
  return invokeDesktop("start_recording", { pauseBackgroundAudio });
}

export async function stopRecording(): Promise<RecordedAudio> {
  return invokeDesktop<RecordedAudio>("stop_recording");
}

export async function transcribeAudio(
  audioBase64: string,
  mode: DictationMode,
  appName?: string | null,
): Promise<TranscriptionResponse> {
  return invokeDesktop<TranscriptionResponse>("transcribe_audio", {
    audioBase64,
    mode,
    appName,
  });
}

export async function cleanupText(
  rawText: string,
  mode: DictationMode,
  appName?: string | null,
  preserveRaw = false,
): Promise<CleanupResponse> {
  return invokeDesktop<CleanupResponse>("cleanup_text", {
    rawText,
    mode,
    appName,
    preserveRaw,
  });
}

export async function getModelStatus(): Promise<ModelStatus> {
  return invokeDesktop<ModelStatus>("get_model_status");
}

export async function downloadModel(): Promise<void> {
  return invokeDesktop("download_model");
}

export async function openMicrophoneSettings(): Promise<void> {
  return invokeDesktop("open_microphone_settings");
}

export async function openAccessibilitySettings(): Promise<void> {
  return invokeDesktop("open_accessibility_settings");
}

export async function requestMicrophoneAccess(): Promise<void> {
  return invokeDesktop("request_microphone_access");
}

export async function requestAccessibilityAccess(): Promise<void> {
  return invokeDesktop("request_accessibility_access");
}

export async function getFrontmostApp(): Promise<string | null> {
  return invokeDesktop<string | null>("get_frontmost_app");
}

export async function captureClipboard(): Promise<ClipboardSnapshot> {
  return invokeDesktop<ClipboardSnapshot>("capture_clipboard");
}

export async function setClipboardText(text: string): Promise<void> {
  return invokeDesktop("set_clipboard_text", { text });
}

export async function restoreClipboard(
  snapshot: ClipboardSnapshot,
): Promise<void> {
  return invokeDesktop("restore_clipboard", { snapshot });
}

export async function simulatePaste(): Promise<void> {
  return invokeDesktop("simulate_paste");
}

export async function simulateEnter(): Promise<void> {
  return invokeDesktop("simulate_enter");
}

export async function quitApp(): Promise<void> {
  return invokeDesktop("quit_app");
}

export async function getHudState(): Promise<HudState> {
  return invokeDesktop<HudState>("get_hud_state");
}

export async function showHud(phase: Exclude<HudPhase, "hidden">): Promise<void> {
  return invokeDesktop("show_hud_command", { phase });
}

export async function setHudPhase(
  phase: Exclude<HudPhase, "hidden">,
): Promise<void> {
  return invokeDesktop("set_hud_phase_command", { phase });
}

export async function hideHud(): Promise<void> {
  return invokeDesktop("hide_hud_command");
}

export async function registerGlobalShortcut(
  shortcut: string,
  handlers: {
    onPressed: () => void;
    onReleased?: () => void;
  },
): Promise<void> {
  await register(shortcut, (event) => {
    if (event.state === "Pressed") {
      handlers.onPressed();
    }

    if (event.state === "Released" && handlers.onReleased) {
      handlers.onReleased();
    }
  });
}

export async function unregisterGlobalShortcut(shortcut: string): Promise<void> {
  await unregister(shortcut);
}

export async function unregisterAllGlobalShortcuts(): Promise<void> {
  await unregisterAll();
}

export async function showMainWindow(): Promise<void> {
  await show();
  const window = getCurrentWindow();
  await window.show();
  await window.unminimize();
  await window.setFocus();
}

export async function onTrayToggleRecording(
  handler: () => void,
): Promise<() => void> {
  if (!isTauri()) {
    return () => undefined;
  }

  return listen("tray-toggle-recording", () => {
    handler();
  });
}

export async function onTrayModeChange(
  handler: (mode: DictationMode) => void,
): Promise<() => void> {
  if (!isTauri()) {
    return () => undefined;
  }

  return listen<DictationMode>("tray-mode-changed", (event) => {
    handler(event.payload);
  });
}

export async function onTraySettingsChange(
  handler: (settings: Settings) => void,
): Promise<() => void> {
  if (!isTauri()) {
    return () => undefined;
  }

  return listen<Settings>("tray-settings-changed", (event) => {
    handler(event.payload);
  });
}

export async function onCurrentWindowFocusChange(
  handler: (focused: boolean) => void,
): Promise<() => void> {
  if (!isTauri()) {
    const handleFocus = () => {
      handler(true);
    };
    const handleBlur = () => {
      handler(false);
    };

    window.addEventListener("focus", handleFocus);
    window.addEventListener("blur", handleBlur);

    return () => {
      window.removeEventListener("focus", handleFocus);
      window.removeEventListener("blur", handleBlur);
    };
  }

  return getCurrentWindow().onFocusChanged(({ payload }) => {
    handler(payload);
  });
}

export function currentDesktopWindowLabel(): string {
  if (!isTauri()) {
    return "main";
  }

  return getCurrentWebviewWindow().label;
}

export async function onHudStateChange(
  handler: (state: HudState) => void,
): Promise<() => void> {
  if (!isTauri()) {
    return () => undefined;
  }

  return listen<HudState>("hud-state-changed", (event) => {
    handler(event.payload);
  });
}

export async function onHudMeter(
  handler: (payload: HudMeterPayload) => void,
): Promise<() => void> {
  if (!isTauri()) {
    return () => undefined;
  }

  return listen<HudMeterPayload>("hud-meter", (event) => {
    handler(event.payload);
  });
}
