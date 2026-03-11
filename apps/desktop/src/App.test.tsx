import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import App from "./App";
import * as desktop from "@/lib/desktop";
import * as recordingCues from "@/lib/recording-cues";

const defaultSettings = {
  hotkey: "Option+Space",
  autoPaste: true,
  pauseBackgroundAudio: true,
  recordingCueVolume: 75,
  restoreClipboard: true,
  interactionMode: "hold" as const,
  preferredMode: "default" as const,
  saveHistory: true,
};

const defaultPermissions = {
  microphone: "granted" as const,
  accessibility: "granted" as const,
};

const defaultModelStatus = {
  installed: true,
  downloading: false,
  progress: null,
  error: null,
};

function mockSuccessfulBootstrap() {
  vi.spyOn(desktop, "getSettings").mockResolvedValue(defaultSettings);
  vi.spyOn(desktop, "getPermissionStatus").mockResolvedValue(defaultPermissions);
  vi.spyOn(desktop, "getModelStatus").mockResolvedValue(defaultModelStatus);
  vi.spyOn(desktop, "downloadModel").mockResolvedValue(undefined);
  vi.spyOn(desktop, "listHistory").mockResolvedValue([]);
  vi.spyOn(desktop, "updateSettings").mockImplementation(async (settings) => settings);
  vi.spyOn(desktop, "registerGlobalShortcut").mockResolvedValue(undefined);
  vi.spyOn(desktop, "unregisterGlobalShortcut").mockResolvedValue(undefined);
  vi.spyOn(desktop, "unregisterAllGlobalShortcuts").mockResolvedValue(undefined);
  vi.spyOn(desktop, "onTrayToggleRecording").mockResolvedValue(() => undefined);
  vi.spyOn(desktop, "showMainWindow").mockResolvedValue(undefined);
  vi.spyOn(desktop, "quitApp").mockResolvedValue(undefined);
  vi.spyOn(desktop, "startRecording").mockResolvedValue(undefined);
  vi.spyOn(desktop, "stopRecording").mockResolvedValue({
    audioBase64: btoa("fake-wav"),
    durationMs: 450,
    sampleRate: 16000,
  });
  vi.spyOn(desktop, "getFrontmostApp").mockResolvedValue("TextEdit");
  vi.spyOn(desktop, "captureClipboard").mockResolvedValue({ text: "before" });
  vi.spyOn(desktop, "setClipboardText").mockResolvedValue(undefined);
  vi.spyOn(desktop, "restoreClipboard").mockResolvedValue(undefined);
  vi.spyOn(desktop, "simulatePaste").mockResolvedValue(undefined);
  vi.spyOn(desktop, "transcribeAudio").mockResolvedValue({
    raw_text: "hey can you send the update tomorrow",
    language: "en",
    duration_ms: 450,
    provider: "mock",
  });
  vi.spyOn(desktop, "cleanupText").mockResolvedValue({
    clean_text: "Hey, can you send the update tomorrow?",
    provider: "mock",
  });
  vi.spyOn(desktop, "createHistoryRecord").mockImplementation(async (record) => ({
    id: "history-1",
    createdAt: "2026-03-11T00:00:00Z",
    ...record,
    appName: record.appName ?? null,
  }));
  vi.spyOn(recordingCues, "playRecordingStartCue").mockResolvedValue(undefined);
  vi.spyOn(recordingCues, "playRecordingStopCue").mockResolvedValue(undefined);
}

describe("App", () => {
  beforeEach(() => {
    vi.restoreAllMocks();
    mockSuccessfulBootstrap();
  });

  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it("moves from idle to recording and back through processing", async () => {
    render(<App />);

    const holdButton = await screen.findByRole("button", {
      name: "Hold to Record",
    });

    fireEvent.mouseDown(holdButton);
    expect(await screen.findByRole("button", { name: "Release to Transcribe" })).toBeVisible();
    expect(desktop.startRecording).toHaveBeenCalledWith(true);
    expect(recordingCues.playRecordingStartCue).toHaveBeenCalledWith(75);

    fireEvent.mouseUp(screen.getByRole("button", { name: "Release to Transcribe" }));

    await waitFor(() => {
      expect(screen.getByText("Inserted")).toBeVisible();
    });
    expect(recordingCues.playRecordingStopCue).toHaveBeenCalledWith(75);

    expect(
      screen.getAllByText("Hey, can you send the update tomorrow?").length,
    ).toBeGreaterThan(0);
  });

  it("shows an error banner when the API call fails", async () => {
    vi.spyOn(desktop, "transcribeAudio").mockRejectedValue(new Error("upstream failed"));

    render(<App />);

    const holdButton = await screen.findByRole("button", { name: "Hold to Record" });
    fireEvent.mouseDown(holdButton);
    fireEvent.mouseUp(await screen.findByRole("button", { name: "Release to Transcribe" }));

    expect(await screen.findByText("upstream failed")).toBeVisible();
  });

  it("shows an error banner when transcription returns empty text", async () => {
    vi.spyOn(desktop, "transcribeAudio").mockResolvedValue({
      raw_text: "   ",
      language: "en",
      duration_ms: 450,
      provider: "mock",
    });

    render(<App />);

    const holdButton = await screen.findByRole("button", { name: "Hold to Record" });
    fireEvent.mouseDown(holdButton);
    fireEvent.mouseUp(await screen.findByRole("button", { name: "Release to Transcribe" }));

    expect(
      await screen.findByText(
        "No speech was detected in the recording. Try speaking louder or checking the selected microphone.",
      ),
    ).toBeVisible();
  });

  it("opens the fallback panel when paste fails", async () => {
    vi.spyOn(desktop, "transcribeAudio").mockResolvedValue({
      raw_text: "running five minutes late be there soon",
      language: "en",
      duration_ms: 450,
      provider: "mock",
    });
    vi.spyOn(desktop, "cleanupText").mockResolvedValue({
      clean_text: "Running five minutes late. Be there soon.",
      provider: "mock",
    });
    vi.spyOn(desktop, "simulatePaste").mockRejectedValue(new Error("Accessibility denied"));

    render(<App />);

    const holdButton = await screen.findByRole("button", { name: "Hold to Record" });
    fireEvent.mouseDown(holdButton);
    fireEvent.mouseUp(await screen.findByRole("button", { name: "Release to Transcribe" }));

    expect(await screen.findByLabelText("Fallback paste panel")).toBeVisible();
    expect(screen.getByText("Accessibility denied")).toBeVisible();
    expect(
      screen.getAllByText("Running five minutes late. Be there soon.").length,
    ).toBeGreaterThan(0);
  });

  it("subscribes to native tray events once the app boots", async () => {
    render(<App />);

    await waitFor(() => {
      expect(desktop.onTrayToggleRecording).toHaveBeenCalledTimes(1);
    });
  });

  it("lets users disable background audio pausing", async () => {
    render(<App />);

    const checkbox = await screen.findByLabelText(
      "Pause background audio during recording",
    );
    fireEvent.click(checkbox);

    await waitFor(() => {
      expect(desktop.updateSettings).toHaveBeenCalledWith(
        expect.objectContaining({ pauseBackgroundAudio: false }),
      );
    });

    fireEvent.mouseDown(await screen.findByRole("button", { name: "Hold to Record" }));

    await waitFor(() => {
      expect(desktop.startRecording).toHaveBeenLastCalledWith(false);
    });
  });

  it("persists recording cue volume changes from the settings slider", async () => {
    render(<App />);

    const slider = await screen.findByLabelText("Recording cue volume (75%)");
    fireEvent.change(slider, { target: { value: "0", valueAsNumber: 0 } });

    await waitFor(() => {
      expect(desktop.updateSettings).toHaveBeenCalledWith(
        expect.objectContaining({ recordingCueVolume: 0 }),
      );
    });
  });

  it("supports toggle mode when configured in settings", async () => {
    vi.spyOn(desktop, "getSettings").mockResolvedValue({
      ...defaultSettings,
      interactionMode: "toggle",
    });
    vi.spyOn(desktop, "transcribeAudio").mockResolvedValue({
      raw_text: "follow up tomorrow morning",
      language: "en",
      duration_ms: 450,
      provider: "mock",
    });
    vi.spyOn(desktop, "cleanupText").mockResolvedValue({
      clean_text: "Follow up tomorrow morning.",
      provider: "mock",
    });

    render(<App />);

    const toggleButton = await screen.findByRole("button", { name: "Start Recording" });
    fireEvent.click(toggleButton);
    expect(await screen.findByRole("button", { name: "Stop Recording" })).toBeVisible();

    fireEvent.click(screen.getByRole("button", { name: "Stop Recording" }));

    await waitFor(() => {
      expect(screen.getByText("Inserted")).toBeVisible();
    });
  });

  it("offers a model download action when the local model is missing", async () => {
    vi.spyOn(desktop, "getModelStatus").mockResolvedValue({
      installed: false,
      downloading: false,
      progress: null,
      error: null,
    });

    render(<App />);

    const button = await screen.findByRole("button", { name: "Download Model" });
    fireEvent.click(button);

    await waitFor(() => {
      expect(desktop.downloadModel).toHaveBeenCalledTimes(1);
    });
  });
});
