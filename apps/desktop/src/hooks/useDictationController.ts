import { useEffect, useRef, useState } from "react";
import type {
  DictationMode,
  HistoryRecord,
  NewHistoryRecord,
  PermissionStatus,
  Settings,
} from "@tator/shared-types";
import { createApiClient } from "@/lib/api-client";
import {
  captureClipboard,
  createHistoryRecord,
  getFrontmostApp,
  getPermissionStatus,
  getSettings,
  listHistory,
  quitApp,
  registerGlobalShortcut,
  restoreClipboard,
  setClipboardText,
  showMainWindow,
  simulatePaste,
  startRecording,
  stopRecording,
  unregisterAllGlobalShortcuts,
  unregisterGlobalShortcut,
  updateSettings,
} from "@/lib/desktop";
import { ensureTray } from "@/lib/tray";

export type WorkflowStatus =
  | "booting"
  | "idle"
  | "recording"
  | "processing"
  | "success"
  | "error";

const DEFAULT_SETTINGS: Settings = {
  hotkey: "Option+Space",
  apiBaseUrl: import.meta.env.VITE_API_BASE_URL ?? "http://127.0.0.1:8000",
  autoPaste: true,
  pauseBackgroundAudio: true,
  restoreClipboard: true,
  interactionMode: "hold",
  preferredMode: "default",
  saveHistory: true,
};

const DEFAULT_PERMISSIONS: PermissionStatus = {
  microphone: "unknown",
  accessibility: "unknown",
};

type ControllerState = {
  status: WorkflowStatus;
  settings: Settings;
  permissions: PermissionStatus;
  mode: DictationMode;
  history: HistoryRecord[];
  lastRawText: string | null;
  lastCleanText: string | null;
  fallbackText: string | null;
  error: string | null;
  initialized: boolean;
};

function describeError(error: unknown, fallback: string): string {
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
      return fallback;
    }
  }

  return fallback;
}

async function pause(milliseconds: number): Promise<void> {
  await new Promise((resolve) => window.setTimeout(resolve, milliseconds));
}

async function pasteText(text: string, restoreOriginalClipboard: boolean) {
  const snapshot = await captureClipboard();

  try {
    await setClipboardText(text);
    await pause(60);
    await simulatePaste();
  } finally {
    if (restoreOriginalClipboard) {
      await pause(60);
      await restoreClipboard(snapshot);
    }
  }
}

export function useDictationController() {
  const [status, setStatus] = useState<WorkflowStatus>("booting");
  const [settings, setSettings] = useState<Settings>(DEFAULT_SETTINGS);
  const [permissions, setPermissions] =
    useState<PermissionStatus>(DEFAULT_PERMISSIONS);
  const [mode, setMode] = useState<DictationMode>(DEFAULT_SETTINGS.preferredMode);
  const [history, setHistory] = useState<HistoryRecord[]>([]);
  const [lastRawText, setLastRawText] = useState<string | null>(null);
  const [lastCleanText, setLastCleanText] = useState<string | null>(null);
  const [fallbackText, setFallbackText] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [initialized, setInitialized] = useState(false);

  const stateRef = useRef<ControllerState>({
    status,
    settings,
    permissions,
    mode,
    history,
    lastRawText,
    lastCleanText,
    fallbackText,
    error,
    initialized,
  });

  useEffect(() => {
    stateRef.current = {
      status,
      settings,
      permissions,
      mode,
      history,
      lastRawText,
      lastCleanText,
      fallbackText,
      error,
      initialized,
    };
  }, [
    error,
    fallbackText,
    history,
    initialized,
    lastCleanText,
    lastRawText,
    mode,
    permissions,
    settings,
    status,
  ]);

  useEffect(() => {
    let disposed = false;

    async function bootstrap() {
      try {
        const [loadedSettings, loadedPermissions, loadedHistory] =
          await Promise.all([
            getSettings(),
            getPermissionStatus(),
            listHistory(8),
          ]);

        if (disposed) {
          return;
        }

        stateRef.current = {
          ...stateRef.current,
          settings: loadedSettings,
          permissions: loadedPermissions,
          mode: loadedSettings.preferredMode,
          history: loadedHistory,
          status: "idle",
          initialized: true,
        };
        setSettings(loadedSettings);
        setMode(loadedSettings.preferredMode);
        setPermissions(loadedPermissions);
        setHistory(loadedHistory);
        setStatus("idle");
        setInitialized(true);

        try {
          await ensureTray({
            onShowWindow: () => {
              void showMainWindow();
            },
            onToggleRecording: () => {
              void toggleRecording();
            },
            onQuit: () => {
              void quitApp();
            },
          });
        } catch (trayError) {
          if (!disposed) {
            setError(
              `Tray initialization failed: ${describeError(
                trayError,
                "Unknown tray error.",
              )}`,
            );
          }
        }
      } catch (bootstrapError) {
        if (disposed) {
          return;
        }

        setStatus("error");
        setError(describeError(bootstrapError, "Unable to bootstrap the desktop app."));
      }
    }

    void bootstrap();

    return () => {
      disposed = true;
      void unregisterAllGlobalShortcuts();
    };
  }, []);

  useEffect(() => {
    if (!initialized) {
      return;
    }

    let active = true;

    async function syncShortcut() {
      try {
        await unregisterAllGlobalShortcuts();
        if (settings.interactionMode === "toggle") {
          await registerGlobalShortcut(settings.hotkey, {
            onPressed: () => {
              void toggleRecording();
            },
          });
        } else {
          await registerGlobalShortcut(settings.hotkey, {
            onPressed: () => {
              void startHoldRecording();
            },
            onReleased: () => {
              void stopHoldRecording();
            },
          });
        }
      } catch (registrationError) {
        if (!active) {
          return;
        }

        setError(
          describeError(
            registrationError,
            "Failed to register the global shortcut.",
          ),
        );
      }
    }

    void syncShortcut();

    return () => {
      active = false;
      void unregisterGlobalShortcut(settings.hotkey).catch(() => undefined);
    };
  }, [initialized, settings.hotkey, settings.interactionMode]);

  async function refreshPermissions() {
    try {
      setPermissions(await getPermissionStatus());
    } catch {
      // Permission status is best-effort; keep the UI responsive even if it fails.
    }
  }

  async function appendHistory(record: NewHistoryRecord) {
    if (!stateRef.current.settings.saveHistory) {
      return;
    }

    const savedRecord = await createHistoryRecord(record);
    setHistory((current) => [savedRecord, ...current].slice(0, 8));
  }

  async function beginRecording() {
    stateRef.current.status = "recording";
    try {
      setError(null);
      setFallbackText(null);
      setStatus("recording");
      await startRecording(stateRef.current.settings.pauseBackgroundAudio);
      await refreshPermissions();
    } catch (startError) {
      stateRef.current.status = "error";
      setStatus("error");
      setError(describeError(startError, "Unable to start recording."));
    }
  }

  async function finishRecording() {
    stateRef.current.status = "processing";
    try {
      setStatus("processing");
      setError(null);

      const activeState = stateRef.current;
      const audio = await stopRecording();
      const appName = await getFrontmostApp();
      const client = createApiClient(activeState.settings.apiBaseUrl);
      const transcription = await client.transcribe(
        audio.audioBase64,
        activeState.mode,
        appName,
      );
      const cleanup = await client.cleanup({
        raw_text: transcription.raw_text,
        mode: activeState.mode,
        app_name: appName,
      });

      setLastRawText(transcription.raw_text);
      setLastCleanText(cleanup.clean_text);

      let historyStatus: NewHistoryRecord["status"] = "inserted";

      if (activeState.settings.autoPaste) {
        try {
          await pasteText(
            cleanup.clean_text,
            activeState.settings.restoreClipboard,
          );
        } catch (pasteError) {
          historyStatus = "fallback";
          setFallbackText(cleanup.clean_text);
          setStatus("error");
          setError(
            describeError(
              pasteError,
              "Paste failed. Use the fallback panel instead.",
            ),
          );
        }
      }

      await appendHistory({
        appName,
        mode: activeState.mode,
        audioDurationMs: audio.durationMs,
        rawText: transcription.raw_text,
        cleanText: cleanup.clean_text,
        status: historyStatus,
      });

      if (historyStatus === "inserted") {
        stateRef.current.status = "success";
        setStatus("success");
      }
    } catch (processingError) {
      stateRef.current.status = "error";
      setStatus("error");
      setError(describeError(processingError, "Unable to finish dictation."));
    } finally {
      await refreshPermissions();
    }
  }

  async function startHoldRecording() {
    const activeState = stateRef.current;
    if (
      activeState.status === "booting" ||
      activeState.status === "processing" ||
      activeState.status === "recording"
    ) {
      return;
    }

    await beginRecording();
  }

  async function stopHoldRecording() {
    if (stateRef.current.status !== "recording") {
      return;
    }

    await finishRecording();
  }

  async function toggleRecording() {
    const activeState = stateRef.current;
    if (activeState.status === "processing") {
      return;
    }

    if (activeState.status === "recording") {
      await finishRecording();
      return;
    }

    await beginRecording();
  }

  async function saveSettings(nextSettings: Settings) {
    const saved = await updateSettings(nextSettings);
    stateRef.current = {
      ...stateRef.current,
      settings: saved,
      mode: saved.preferredMode,
    };
    setSettings(saved);
    setMode(saved.preferredMode);
  }

  async function retryPaste() {
    if (!stateRef.current.fallbackText) {
      return;
    }

    try {
      await pasteText(
        stateRef.current.fallbackText,
        stateRef.current.settings.restoreClipboard,
      );
      setStatus("success");
      setError(null);
      setFallbackText(null);
    } catch (retryError) {
      setStatus("error");
      setError(describeError(retryError, "Retry paste failed."));
    }
  }

  async function copyFallbackText() {
    if (!stateRef.current.fallbackText) {
      return;
    }

    await setClipboardText(stateRef.current.fallbackText);
  }

  return {
    status,
    settings,
    permissions,
    mode,
    history,
    lastRawText,
    lastCleanText,
    fallbackText,
    error,
    initialized,
    setMode,
    saveSettings,
    toggleRecording,
    startHoldRecording,
    stopHoldRecording,
    retryPaste,
    copyFallbackText,
  };
}
