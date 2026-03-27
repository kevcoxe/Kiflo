export const DICTATION_MODES = [
  "default",
  "chat",
  "bullet_list",
  "auto",
] as const;

export type DictationMode = (typeof DICTATION_MODES)[number];

export const TYPING_STYLES = [
  "very_casual",
  "formal",
  "excited",
] as const;

export type TypingStyle = (typeof TYPING_STYLES)[number];

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

export const SPEECH_MODELS = [
  "base_en",
  "small_en",
] as const;

export type SpeechModel = (typeof SPEECH_MODELS)[number];

export const THEMES = ["system", "light", "dark"] as const;

export type Theme = (typeof THEMES)[number];

export interface Settings {
  hotkey: string;
  autoPaste: boolean;
  autoBacktrack: boolean;
  editorFileMentionsEnabled: boolean;
  forceEditorComposer: boolean;
  spokenPressEnterCommand: boolean;
  pauseBackgroundAudio: boolean;
  warmModelOnLaunch: boolean;
  speechModel: SpeechModel;
  inputDeviceId: string;
  recordingCueVolume: number;
  restoreClipboard: boolean;
  interactionMode: RecordingInteractionMode;
  preferredMode: DictationMode;
  typingStyle: TypingStyle;
  customDictionary: string;
  saveHistory: boolean;
  hideDockIcon: boolean;
  showHud: boolean;
  showEditorDebugHud: boolean;
  theme: Theme;
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
  insertion_plan?: CleanupInsertionSegment[];
  provider: string;
}

export type CleanupInsertionSegment =
  | {
      kind: "text";
      text: string;
    }
  | {
      kind: "file_mention";
      query: string;
      fallbackText: string;
    };

export interface FocusedEditorContext {
  appName: string | null;
  isSupportedEditor: boolean;
  isTextInputLike: boolean;
  isComposer: boolean;
  confidence: number;
  matchedPositiveCues: string[];
  matchedNegativeCues: string[];
  rawRoleValues: string[];
  rawTextValues: string[];
}

export interface ModelStatus {
  installed: boolean;
  bundledModels: SpeechModel[];
  activeModel: SpeechModel;
  installedModels: SpeechModel[];
  downloadTarget?: SpeechModel | null;
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

export interface InputDevice {
  id: string;
  name: string;
  isDefault: boolean;
}
