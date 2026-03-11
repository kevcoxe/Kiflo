import { invoke } from "@tauri-apps/api/core";
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

export async function getSettings(): Promise<Settings> {
  return invoke<Settings>("get_settings");
}

export async function updateSettings(settings: Settings): Promise<Settings> {
  return invoke<Settings>("update_settings", { settings });
}

export async function listHistory(limit = 10): Promise<HistoryRecord[]> {
  return invoke<HistoryRecord[]>("list_history", { limit });
}

export async function createHistoryRecord(
  record: NewHistoryRecord,
): Promise<HistoryRecord> {
  return invoke<HistoryRecord>("create_history_record", { record });
}

export async function getPermissionStatus(): Promise<PermissionStatus> {
  return invoke<PermissionStatus>("get_permission_status");
}

export async function startRecording(): Promise<void> {
  return invoke("start_recording");
}

export async function stopRecording(): Promise<RecordedAudio> {
  return invoke<RecordedAudio>("stop_recording");
}

export async function getFrontmostApp(): Promise<string | null> {
  return invoke<string | null>("get_frontmost_app");
}

export async function captureClipboard(): Promise<ClipboardSnapshot> {
  return invoke<ClipboardSnapshot>("capture_clipboard");
}

export async function setClipboardText(text: string): Promise<void> {
  return invoke("set_clipboard_text", { text });
}

export async function restoreClipboard(
  snapshot: ClipboardSnapshot,
): Promise<void> {
  return invoke("restore_clipboard", { snapshot });
}

export async function simulatePaste(): Promise<void> {
  return invoke("simulate_paste");
}

export async function quitApp(): Promise<void> {
  return invoke("quit_app");
}

export async function registerGlobalShortcut(
  shortcut: string,
  handler: () => void,
): Promise<void> {
  await register(shortcut, (event) => {
    if (event.state === "Pressed") {
      handler();
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
