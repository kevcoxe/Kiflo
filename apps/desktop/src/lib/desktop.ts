import { invoke, isTauri } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import {
  register,
  unregister,
  unregisterAll,
} from "@tauri-apps/plugin-global-shortcut";
import type {
  ClipboardSnapshot,
  HistoryRecord,
  NewHistoryRecord,
  PermissionStatus,
  RecordedAudio,
  Settings,
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

export async function quitApp(): Promise<void> {
  return invokeDesktop("quit_app");
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
  const window = getCurrentWindow();
  await window.show();
  await window.unminimize();
  await window.setFocus();
}
