export const DICTATION_MODES = [
  "default",
  "chat",
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
  autoPaste: boolean;
  autoBacktrack: boolean;
  spokenPressEnterCommand: boolean;
  pauseBackgroundAudio: boolean;
  warmModelOnLaunch: boolean;
  recordingCueVolume: number;
  restoreClipboard: boolean;
  interactionMode: RecordingInteractionMode;
  preferredMode: DictationMode;
  customDictionary: string;
  saveHistory: boolean;
  hideDockIcon: boolean;
  showHud: boolean;
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

export interface HistoryWordMetric {
  word: string;
  count: number;
}

export interface HistoryMetrics {
  totalSessions: number;
  totalWordsSpoken: number;
  totalSpeakingTimeMs: number;
  estimatedTypingTimeSavedMs: number;
  averageWordsPerSession: number;
  topWords: HistoryWordMetric[];
  topMeaningfulWords: HistoryWordMetric[];
}

export interface TranscriptionResponse {
  raw_text: string;
  language: string;
  duration_ms: number;
  provider: string;
}

export interface CleanupResponse {
  clean_text: string;
  provider: string;
}

export interface ModelStatus {
  installed: boolean;
  downloading: boolean;
  progress?: number | null;
  error?: string | null;
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
