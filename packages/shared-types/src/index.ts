export const DICTATION_MODES = [
  "default",
  "email",
  "chat",
  "notes",
  "bullet_list",
] as const;

export type DictationMode = (typeof DICTATION_MODES)[number];

export const RECORDING_INTERACTION_MODES = ["hold", "toggle"] as const;

export type RecordingInteractionMode =
  (typeof RECORDING_INTERACTION_MODES)[number];

export const HISTORY_STATUSES = [
  "recording",
  "processing",
  "inserted",
  "fallback",
  "failed",
] as const;

export type HistoryStatus = (typeof HISTORY_STATUSES)[number];

export type PermissionState = "granted" | "denied" | "unknown";

export interface Settings {
  hotkey: string;
  apiBaseUrl: string;
  autoPaste: boolean;
  pauseBackgroundAudio: boolean;
  recordingCueVolume: number;
  restoreClipboard: boolean;
  interactionMode: RecordingInteractionMode;
  preferredMode: DictationMode;
  saveHistory: boolean;
}

export interface HistoryRecord {
  id: string;
  createdAt: string;
  appName: string | null;
  mode: DictationMode;
  audioDurationMs: number;
  rawText: string;
  cleanText: string;
  status: HistoryStatus;
}

export interface NewHistoryRecord {
  appName?: string | null;
  mode: DictationMode;
  audioDurationMs: number;
  rawText: string;
  cleanText: string;
  status: HistoryStatus;
}

export interface TranscriptionResponse {
  raw_text: string;
  language: string;
  duration_ms: number;
  provider: string;
}

export interface CleanupRequest {
  raw_text: string;
  mode: DictationMode;
  app_name?: string | null;
  preserve_raw?: boolean;
}

export interface CleanupResponse {
  clean_text: string;
  provider: string;
}

export interface PermissionStatus {
  microphone: PermissionState;
  accessibility: PermissionState;
}

export interface RecordedAudio {
  audioBase64: string;
  durationMs: number;
  sampleRate: number;
}

export interface ClipboardSnapshot {
  text: string | null;
}
