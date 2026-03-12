import { useEffect, useRef, useState } from "react";
import type {
  DictationMode,
  HistoryRecord,
  ModelStatus,
  NewHistoryRecord,
  PermissionStatus,
  Settings,
} from "@tator/shared-types";
import {
  captureClipboard,
  cleanupText,
  createHistoryRecord,
  downloadModel,
  getFrontmostApp,
  getModelStatus,
  onTrayToggleRecording,
  onTrayModeChange,
  onTraySettingsChange,
  getPermissionStatus,
  getSettings,
  hideHud,
  listHistory,
  setHudPhase,
  registerGlobalShortcut,
  restoreClipboard,
  setClipboardText,
  showHud,
  simulatePaste,
  startRecording,
  stopRecording,
  transcribeAudio,
  unregisterAllGlobalShortcuts,
  unregisterGlobalShortcut,
  updateSettings,
} from "@/lib/desktop";
import {
  playRecordingStartCue,
  playRecordingStopCue,
} from "@/lib/recording-cues";

export type WorkflowStatus =
  | "booting"
  | "idle"
  | "recording"
  | "processing"
  | "success"
  | "error";

const DEFAULT_SETTINGS: Settings = {
  hotkey: "Option+Space",
  autoPaste: true,
  pauseBackgroundAudio: true,
  recordingCueVolume: 75,
  restoreClipboard: true,
  interactionMode: "hold",
  preferredMode: "default",
  saveHistory: true,
  hideDockIcon: false,
  showHud: true,
};

const DEFAULT_PERMISSIONS: PermissionStatus = {
  microphone: "unknown",
  accessibility: "unknown",
};

const DEFAULT_MODEL_STATUS: ModelStatus = {
  installed: false,
  downloading: false,
  progress: null,
  error: null,
};

const HUD_ERROR_FLASH_MS = 450;

type ControllerState = {
  status: WorkflowStatus;
  settings: Settings;
  permissions: PermissionStatus;
  modelStatus: ModelStatus;
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
  const [modelStatus, setModelStatus] = useState<ModelStatus>(DEFAULT_MODEL_STATUS);
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
    modelStatus,
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
      modelStatus,
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
    modelStatus,
    mode,
    permissions,
    settings,
    status,
  ]);

  useEffect(() => {
    let disposed = false;
    let removeTrayToggleListener: () => void = () => undefined;
    let removeTrayModeListener: () => void = () => undefined;
    let removeTraySettingsListener: () => void = () => undefined;

    async function bootstrap() {
      try {
        removeTrayToggleListener = await onTrayToggleRecording(() => {
          void toggleRecording();
        });
        removeTrayModeListener = await onTrayModeChange((nextMode) => {
          const nextSettings = {
            ...stateRef.current.settings,
            preferredMode: nextMode,
          };

          stateRef.current = {
            ...stateRef.current,
            settings: nextSettings,
            mode: nextMode,
          };
          setSettings(nextSettings);
          setMode(nextMode);
        });
        removeTraySettingsListener = await onTraySettingsChange(
          (nextSettings) => {
            stateRef.current = {
              ...stateRef.current,
              settings: nextSettings,
              mode: nextSettings.preferredMode,
            };
            setSettings(nextSettings);
            setMode(nextSettings.preferredMode);
          },
        );

        const [loadedSettings, loadedPermissions, loadedHistory, loadedModelStatus] =
          await Promise.all([
            getSettings(),
            getPermissionStatus(),
            listHistory(8),
            getModelStatus().catch(() => DEFAULT_MODEL_STATUS),
          ]);

        if (disposed) {
          return;
        }

        stateRef.current = {
          ...stateRef.current,
          settings: loadedSettings,
          permissions: loadedPermissions,
          modelStatus: loadedModelStatus,
          mode: loadedSettings.preferredMode,
          history: loadedHistory,
          status: "idle",
          initialized: true,
        };
        setSettings(loadedSettings);
        setMode(loadedSettings.preferredMode);
        setPermissions(loadedPermissions);
        setModelStatus(loadedModelStatus);
        setHistory(loadedHistory);
        setStatus("idle");
        setInitialized(true);
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
      removeTrayToggleListener();
      removeTrayModeListener();
      removeTraySettingsListener();
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

  useEffect(() => {
    if (!initialized || settings.showHud) {
      return;
    }

    void safelyHideHud();
  }, [initialized, settings.showHud]);

  async function refreshPermissions() {
    try {
      setPermissions(await getPermissionStatus());
    } catch {
      // Permission status is best-effort; keep the UI responsive even if it fails.
    }
  }

  async function refreshModelState() {
    try {
      const nextStatus = await getModelStatus();
      stateRef.current = {
        ...stateRef.current,
        modelStatus: nextStatus,
      };
      setModelStatus(nextStatus);
    } catch {
      // Model state is best-effort to keep the UI responsive.
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
      void safelyShowHud("recording");
      void playRecordingStartCue(stateRef.current.settings.recordingCueVolume);
      await refreshPermissions();
    } catch (startError) {
      stateRef.current.status = "error";
      setStatus("error");
      setError(describeError(startError, "Unable to start recording."));
      void flashHudError();
    }
  }

  async function finishRecording() {
    stateRef.current.status = "processing";
    try {
      setStatus("processing");
      setError(null);
      void safelySetHudPhase("processing");

      const activeState = stateRef.current;
      const audio = await stopRecording();
      void playRecordingStopCue(activeState.settings.recordingCueVolume);
      const appName = await getFrontmostApp();
      const transcription = await transcribeAudio(
        audio.audioBase64,
        activeState.mode,
        appName,
      );
      const cleanup = await cleanupText(
        transcription.raw_text,
        activeState.mode,
        appName,
      );
      const rawText = transcription.raw_text.trim();
      const cleanText = cleanup.clean_text.trim();
      if (!rawText) {
        throw new Error(
          "No speech was detected in the recording. Try speaking louder or checking the selected microphone.",
        );
      }
      if (!cleanText) {
        throw new Error(
          "The cleanup step returned empty text. Try again or switch recording modes.",
        );
      }

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
        void safelyHideHud();
        stateRef.current.status = "success";
        setStatus("success");
      } else {
        void flashHudError();
      }
    } catch (processingError) {
      stateRef.current.status = "error";
      setStatus("error");
      setError(describeError(processingError, "Unable to finish dictation."));
      void flashHudError();
    } finally {
      await refreshPermissions();
      await refreshModelState();
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

  async function installModel() {
    try {
      setError(null);
      setModelStatus((current) => ({
        ...current,
        downloading: true,
        error: null,
      }));
      await downloadModel();
    } catch (downloadError) {
      setError(
        describeError(
          downloadError,
          "Unable to install the bundled local speech model.",
        ),
      );
    } finally {
      await refreshModelState();
    }
  }

  async function safelyShowHud(phase: "recording" | "processing" | "error") {
    if (!stateRef.current.settings.showHud) {
      return;
    }

    try {
      await showHud(phase);
    } catch {
      // HUD failures should not block dictation.
    }
  }

  async function safelySetHudPhase(phase: "recording" | "processing" | "error") {
    if (!stateRef.current.settings.showHud) {
      return;
    }

    try {
      await setHudPhase(phase);
    } catch {
      // HUD failures should not block dictation.
    }
  }

  async function safelyHideHud() {
    try {
      await hideHud();
    } catch {
      // HUD failures should not block dictation.
    }
  }

  async function flashHudError() {
    if (!stateRef.current.settings.showHud) {
      return;
    }

    try {
      await setHudPhase("error");
      await pause(HUD_ERROR_FLASH_MS);
      await hideHud();
    } catch {
      // HUD failures should not block dictation.
    }
  }

  return {
    status,
    settings,
    permissions,
    modelStatus,
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
    downloadModel: installModel,
  };
}
