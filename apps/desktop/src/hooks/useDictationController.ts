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
  onCurrentWindowFocusChange,
  openAccessibilitySettings,
  openMicrophoneSettings,
  getPermissionStatus,
  getSettings,
  hideHud,
  listHistory,
  setHudPhase,
  registerGlobalShortcut,
  restoreClipboard,
  requestAccessibilityAccess,
  requestMicrophoneAccess,
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
  warmModelOnLaunch: true,
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
const PERMISSION_REQUIRED_MESSAGE =
  "Grant microphone and accessibility permissions before recording.";

type ControllerState = {
  status: WorkflowStatus;
  settings: Settings;
  permissions: PermissionStatus;
  modelStatus: ModelStatus;
  mode: DictationMode;
  history: HistoryRecord[];
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
      fallbackText,
      error,
      initialized,
    };
  }, [
    error,
    fallbackText,
    history,
    initialized,
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
    let removeWindowFocusListener: () => void = () => undefined;

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
        removeWindowFocusListener = await onCurrentWindowFocusChange((focused) => {
          if (!focused) {
            return;
          }

          void refreshPermissions();
        });

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
      removeWindowFocusListener();
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
      const nextPermissions = await getPermissionStatus();
      const permissionsReady =
        nextPermissions.microphone === "granted" &&
        nextPermissions.accessibility === "granted";
      const previousError = stateRef.current.error;
      const previousStatus = stateRef.current.status;
      const shouldClearPermissionError =
        permissionsReady && previousError === PERMISSION_REQUIRED_MESSAGE;

      stateRef.current = {
        ...stateRef.current,
        permissions: nextPermissions,
        error: shouldClearPermissionError ? null : previousError,
        status:
          shouldClearPermissionError && previousStatus === "error"
            ? "idle"
            : previousStatus,
      };

      setPermissions(nextPermissions);

      if (shouldClearPermissionError) {
        setError(null);

        if (previousStatus === "error") {
          setStatus("idle");
        }
      }
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

    if (
      activeState.permissions.microphone !== "granted" ||
      activeState.permissions.accessibility !== "granted"
    ) {
      stateRef.current = {
        ...activeState,
        status: "error",
        error: PERMISSION_REQUIRED_MESSAGE,
      };
      setStatus("error");
      setError(PERMISSION_REQUIRED_MESSAGE);
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

    if (
      activeState.permissions.microphone !== "granted" ||
      activeState.permissions.accessibility !== "granted"
    ) {
      stateRef.current = {
        ...activeState,
        status: "error",
        error: PERMISSION_REQUIRED_MESSAGE,
      };
      setStatus("error");
      setError(PERMISSION_REQUIRED_MESSAGE);
      return;
    }

    await beginRecording();
  }

  async function openMicrophoneSystemSettings() {
    await openMicrophoneSettings();
  }

  async function openAccessibilitySystemSettings() {
    await openAccessibilitySettings();
  }

  async function promptMicrophonePermission() {
    await requestMicrophoneAccess();
    await refreshPermissions();
  }

  async function promptAccessibilityPermission() {
    await requestAccessibilityAccess();
    await refreshPermissions();
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
    refreshPermissions,
    openMicrophoneSystemSettings,
    openAccessibilitySystemSettings,
    promptMicrophonePermission,
    promptAccessibilityPermission,
    downloadModel: installModel,
  };
}
