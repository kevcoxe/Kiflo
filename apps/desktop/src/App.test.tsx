import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import type { DictationMode } from "@tator/shared-types";
import App from "./App";
import * as desktop from "@/lib/desktop";
import * as recordingCues from "@/lib/recording-cues";

const defaultSettings = {
  hotkey: "Option+Space",
  autoPaste: true,
  autoBacktrack: false,
  spokenPressEnterCommand: false,
  pauseBackgroundAudio: true,
  warmModelOnLaunch: true,
  recordingCueVolume: 75,
  restoreClipboard: true,
  interactionMode: "hold" as const,
  preferredMode: "default" as const,
  typingStyle: "formal" as const,
  customDictionary: "",
  saveHistory: true,
  hideDockIcon: false,
  showHud: true,
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


const defaultMetrics = {
  totalSessions: 0,
  totalWordsSpoken: 0,
  totalSpeakingTimeMs: 0,
  estimatedTypingTimeSavedMs: 0,
  averageWordsPerSession: 0,
  topWords: [],
  topMeaningfulWords: [],
};

function mockSuccessfulBootstrap() {
  vi.spyOn(desktop, "getSettings").mockResolvedValue(defaultSettings);
  vi.spyOn(desktop, "getPermissionStatus").mockResolvedValue(defaultPermissions);
  vi.spyOn(desktop, "getModelStatus").mockResolvedValue(defaultModelStatus);
  vi.spyOn(desktop, "downloadModel").mockResolvedValue(undefined);
  vi.spyOn(desktop, "getHistoryMetrics").mockResolvedValue(defaultMetrics);
  vi.spyOn(desktop, "openAccessibilitySettings").mockResolvedValue(undefined);
  vi.spyOn(desktop, "openMicrophoneSettings").mockResolvedValue(undefined);
  vi.spyOn(desktop, "requestMicrophoneAccess").mockResolvedValue(undefined);
  vi.spyOn(desktop, "requestAccessibilityAccess").mockResolvedValue(undefined);
  vi.spyOn(desktop, "listHistory").mockResolvedValue([]);
  vi.spyOn(desktop, "updateSettings").mockImplementation(async (settings) => settings);
  vi.spyOn(desktop, "registerGlobalShortcut").mockResolvedValue(undefined);
  vi.spyOn(desktop, "unregisterGlobalShortcut").mockResolvedValue(undefined);
  vi.spyOn(desktop, "unregisterAllGlobalShortcuts").mockResolvedValue(undefined);
  vi.spyOn(desktop, "onTrayToggleRecording").mockResolvedValue(() => undefined);
  vi.spyOn(desktop, "onTrayModeChange").mockResolvedValue(() => undefined);
  vi.spyOn(desktop, "onCurrentWindowFocusChange").mockResolvedValue(() => undefined);
  vi.spyOn(desktop, "showMainWindow").mockResolvedValue(undefined);
  vi.spyOn(desktop, "quitApp").mockResolvedValue(undefined);
  vi.spyOn(desktop, "getHudState").mockResolvedValue({
    visible: false,
    phase: "hidden",
  });
  vi.spyOn(desktop, "showHud").mockResolvedValue(undefined);
  vi.spyOn(desktop, "setHudPhase").mockResolvedValue(undefined);
  vi.spyOn(desktop, "hideHud").mockResolvedValue(undefined);
  vi.spyOn(desktop, "onHudStateChange").mockResolvedValue(() => undefined);
  vi.spyOn(desktop, "onHudMeter").mockResolvedValue(() => undefined);
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
  vi.spyOn(desktop, "simulateEnter").mockResolvedValue(undefined);
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
    await waitFor(() => {
      expect(desktop.showHud).toHaveBeenCalledWith("recording");
    });

    fireEvent.mouseUp(screen.getByRole("button", { name: "Release to Transcribe" }));

    expect(await screen.findByRole("button", { name: "Processing..." })).toBeVisible();

    await waitFor(() => {
      expect(screen.getByText("Inserted")).toBeVisible();
    });
    expect(desktop.setHudPhase).toHaveBeenCalledWith("processing");
    expect(desktop.hideHud).toHaveBeenCalled();
    expect(recordingCues.playRecordingStopCue).toHaveBeenCalledWith(75);

    expect(
      screen.getAllByText("Hey, can you send the update tomorrow?").length,
    ).toBeGreaterThan(0);
    expect(screen.getByRole("heading", { name: "Dictation metrics" })).toBeVisible();
  });

  it("shows an error banner when the API call fails", async () => {
    vi.spyOn(desktop, "transcribeAudio").mockRejectedValue(new Error("upstream failed"));

    render(<App />);

    const holdButton = await screen.findByRole("button", { name: "Hold to Record" });
    fireEvent.mouseDown(holdButton);
    fireEvent.mouseUp(await screen.findByRole("button", { name: "Release to Transcribe" }));

    expect(await screen.findByText("upstream failed")).toBeVisible();
    await waitFor(() => {
      expect(desktop.setHudPhase).toHaveBeenCalledWith("error");
    });
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
    await waitFor(() => {
      expect(desktop.setHudPhase).toHaveBeenCalledWith("error");
    });
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
    await waitFor(() => {
      expect(desktop.setHudPhase).toHaveBeenCalledWith("error");
    });
  });

  it("subscribes to native tray events once the app boots", async () => {
    render(<App />);

    await waitFor(() => {
      expect(desktop.onTrayToggleRecording).toHaveBeenCalledTimes(1);
      expect(desktop.onTrayModeChange).toHaveBeenCalledTimes(1);
    });
  });

  it("updates the current and preferred mode when the tray menu changes it", async () => {
    let trayModeHandler: ((mode: DictationMode) => void) | undefined;
    vi.spyOn(desktop, "onTrayModeChange").mockImplementation(async (handler) => {
      trayModeHandler = handler as typeof trayModeHandler;
      return () => undefined;
    });

    render(<App />);

    const [modeSelect] = await screen.findAllByRole("combobox");
    const preferredModeSelect = await screen.findByLabelText("Preferred mode");

    act(() => {
      trayModeHandler?.("bullet_list");
    });

    await waitFor(() => {
      expect(modeSelect).toHaveValue("bullet_list");
      expect(preferredModeSelect).toHaveValue("bullet_list");
    });
  });

  it("updates settings checkboxes when the tray menu changes them", async () => {
    let traySettingsHandler:
      | ((settings: typeof defaultSettings) => void)
      | undefined;
    vi.spyOn(desktop, "onTraySettingsChange").mockImplementation(async (handler) => {
      traySettingsHandler = handler as typeof traySettingsHandler;
      return () => undefined;
    });

    render(<App />);

    const autoPasteCheckbox = await screen.findByLabelText(
      "Paste automatically after cleanup",
    );
    const cueCheckbox = await screen.findByLabelText("Play recording cue");
    const autoBacktrackCheckbox = await screen.findByLabelText(
      "Enable Auto Backtrack for spoken corrections",
    );
    const pressEnterCheckbox = await screen.findByLabelText(
      'Enable spoken "press enter" at the end of a recording',
    );
    expect(autoPasteCheckbox).toBeChecked();
    expect(cueCheckbox).toBeChecked();
    expect(autoBacktrackCheckbox).not.toBeChecked();
    expect(pressEnterCheckbox).not.toBeChecked();

    act(() => {
      traySettingsHandler?.({
        ...defaultSettings,
        autoPaste: false,
        autoBacktrack: true,
        spokenPressEnterCommand: true,
        recordingCueVolume: 0,
        restoreClipboard: false,
        saveHistory: false,
        hideDockIcon: true,
        showHud: false,
      });
    });

    await waitFor(() => {
      expect(autoPasteCheckbox).not.toBeChecked();
      expect(cueCheckbox).not.toBeChecked();
      expect(autoBacktrackCheckbox).toBeChecked();
      expect(pressEnterCheckbox).toBeChecked();
      expect(screen.getByLabelText("Restore clipboard after paste")).not.toBeChecked();
      expect(screen.getByLabelText("Save recent dictation history")).not.toBeChecked();
      expect(
        screen.getByLabelText("Hide Dock icon after closing the window"),
      ).toBeChecked();
      expect(screen.getByLabelText("Show recording HUD overlay")).not.toBeChecked();
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

  it("persists the Dock icon preference from settings", async () => {
    render(<App />);

    const checkbox = await screen.findByLabelText(
      "Hide Dock icon after closing the window",
    );
    fireEvent.click(checkbox);

    await waitFor(() => {
      expect(desktop.updateSettings).toHaveBeenCalledWith(
        expect.objectContaining({ hideDockIcon: true }),
      );
    });
  });

  it("persists the typing style preference from settings", async () => {
    render(<App />);

    const select = await screen.findByLabelText(/Typing style/i);
    fireEvent.change(select, { target: { value: "excited" } });

    await waitFor(() => {
      expect(desktop.updateSettings).toHaveBeenCalledWith(
        expect.objectContaining({ typingStyle: "excited" }),
      );
    });
  });

  it("persists the startup model warm-up preference from settings", async () => {
    render(<App />);

    const checkbox = await screen.findByLabelText("Warm speech model on app launch");
    fireEvent.click(checkbox);

    await waitFor(() => {
      expect(desktop.updateSettings).toHaveBeenCalledWith(
        expect.objectContaining({ warmModelOnLaunch: false }),
      );
    });
  });

  it("persists the HUD preference from settings", async () => {
    render(<App />);

    const checkbox = await screen.findByLabelText("Show recording HUD overlay");
    fireEvent.click(checkbox);

    await waitFor(() => {
      expect(desktop.updateSettings).toHaveBeenCalledWith(
        expect.objectContaining({ showHud: false }),
      );
    });
  });

  it("persists custom dictionary replacements from settings", async () => {
    render(<App />);

    fireEvent.click(await screen.findByRole("button", { name: "Add word" }));
    fireEvent.change(await screen.findByLabelText("Dictionary source 1"), {
      target: { value: "Rebecca" },
    });
    fireEvent.change(await screen.findByLabelText("Dictionary replacement 1"), {
      target: { value: "Rebekah" },
    });

    await waitFor(() => {
      expect(desktop.updateSettings).toHaveBeenCalledWith(
        expect.objectContaining({
          customDictionary: "Rebecca => Rebekah",
        }),
      );
    });
  });

  it("persists recording cue playback changes from the settings checkbox", async () => {
    render(<App />);

    const checkbox = await screen.findByLabelText("Play recording cue");
    fireEvent.click(checkbox);

    await waitFor(() => {
      expect(desktop.updateSettings).toHaveBeenCalledWith(
        expect.objectContaining({ recordingCueVolume: 0 }),
      );
    });
  });

  it('persists the spoken "press enter" setting from the settings checkbox', async () => {
    render(<App />);

    const checkbox = await screen.findByLabelText(
      'Enable spoken "press enter" at the end of a recording',
    );
    fireEvent.click(checkbox);

    await waitFor(() => {
      expect(desktop.updateSettings).toHaveBeenCalledWith(
        expect.objectContaining({ spokenPressEnterCommand: true }),
      );
    });
  });

  it("persists the Auto Backtrack setting from the settings checkbox", async () => {
    render(<App />);

    const checkbox = await screen.findByLabelText(
      "Enable Auto Backtrack for spoken corrections",
    );
    fireEvent.click(checkbox);

    await waitFor(() => {
      expect(desktop.updateSettings).toHaveBeenCalledWith(
        expect.objectContaining({ autoBacktrack: true }),
      );
    });
  });

  it('presses Enter when a recording ends with "press enter" and the setting is enabled', async () => {
    vi.spyOn(desktop, "getSettings").mockResolvedValue({
      ...defaultSettings,
      spokenPressEnterCommand: true,
    });
    vi.spyOn(desktop, "transcribeAudio").mockResolvedValue({
      raw_text: "send the update press enter",
      language: "en",
      duration_ms: 450,
      provider: "mock",
    });
    vi.spyOn(desktop, "cleanupText").mockResolvedValue({
      clean_text: "Send the update.",
      provider: "mock",
    });

    render(<App />);

    const holdButton = await screen.findByRole("button", { name: "Hold to Record" });
    fireEvent.mouseDown(holdButton);
    fireEvent.mouseUp(await screen.findByRole("button", { name: "Release to Transcribe" }));

    await waitFor(() => {
      expect(screen.getByText("Inserted")).toBeVisible();
    });

    expect(desktop.cleanupText).toHaveBeenCalledWith(
      "send the update",
      "default",
      "TextEdit",
    );
    expect(desktop.simulatePaste).toHaveBeenCalledTimes(1);
    expect(desktop.simulateEnter).toHaveBeenCalledTimes(1);
    expect(screen.getAllByText("Send the update.").length).toBeGreaterThan(0);
  });

  it('allows a command-only "press enter" recording when the setting is enabled', async () => {
    vi.spyOn(desktop, "getSettings").mockResolvedValue({
      ...defaultSettings,
      spokenPressEnterCommand: true,
    });
    vi.spyOn(desktop, "transcribeAudio").mockResolvedValue({
      raw_text: "press enter",
      language: "en",
      duration_ms: 450,
      provider: "mock",
    });
    vi.spyOn(desktop, "cleanupText").mockResolvedValue({
      clean_text: "",
      provider: "mock",
    });

    render(<App />);

    const holdButton = await screen.findByRole("button", { name: "Hold to Record" });
    fireEvent.mouseDown(holdButton);
    fireEvent.mouseUp(await screen.findByRole("button", { name: "Release to Transcribe" }));

    await waitFor(() => {
      expect(screen.getByText("Inserted")).toBeVisible();
    });

    expect(desktop.simulatePaste).not.toHaveBeenCalled();
    expect(desktop.simulateEnter).toHaveBeenCalledTimes(1);
    expect(
      screen.queryByText(
        "No speech was detected in the recording. Try speaking louder or checking the selected microphone.",
      ),
    ).not.toBeInTheDocument();
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

  it("blocks recording until permissions are granted and offers recovery actions", async () => {
    vi.spyOn(desktop, "getPermissionStatus").mockResolvedValue({
      microphone: "unknown",
      accessibility: "denied",
    });

    render(<App />);

    const disabledButton = await screen.findByRole("button", { name: "Hold to Record" });
    expect(disabledButton).toBeDisabled();
    expect(await screen.findByText("Permissions required to continue")).toBeVisible();

    fireEvent.click(screen.getByRole("button", { name: "Open Microphone Settings" }));
    await waitFor(() => {
      expect(desktop.openMicrophoneSettings).toHaveBeenCalledTimes(1);
    });

    fireEvent.click(screen.getByRole("button", { name: "Open Accessibility Settings" }));
    await waitFor(() => {
      expect(desktop.openAccessibilitySettings).toHaveBeenCalledTimes(1);
    });

    expect(screen.queryByRole("button", { name: "Request Microphone Access" })).toBeNull();
    expect(screen.queryByRole("button", { name: "Request Accessibility Access" })).toBeNull();
  });

  it("collapses the permissions panel when both permissions are granted", async () => {
    render(<App />);

    expect(await screen.findByRole("button", { name: "Hold to Record" })).toBeEnabled();
    expect(screen.getByRole("button", { name: "Show permissions details" })).toBeVisible();
    expect(
      screen.getByText("Microphone and accessibility access are granted."),
    ).toBeVisible();
    expect(screen.queryByText("Microphone")).not.toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "Show permissions details" }));

    expect(await screen.findByText("Microphone")).toBeVisible();
    expect(screen.getByRole("button", { name: "Hide permissions details" })).toBeVisible();
  });

  it("refreshes permissions when the app regains focus and clears the permissions gate", async () => {
    let focusHandler: ((focused: boolean) => void) | undefined;

    vi.spyOn(desktop, "onCurrentWindowFocusChange").mockImplementation(async (handler) => {
      focusHandler = handler;
      return () => undefined;
    });
    vi.spyOn(desktop, "getPermissionStatus")
      .mockResolvedValueOnce({
        microphone: "unknown",
        accessibility: "denied",
      })
      .mockResolvedValueOnce(defaultPermissions);

    render(<App />);

    expect(await screen.findByText("Permissions required to continue")).toBeVisible();

    act(() => {
      focusHandler?.(true);
    });

    await waitFor(() => {
      expect(
        screen.queryByText("Permissions required to continue"),
      ).not.toBeInTheDocument();
    });
    expect(screen.getByRole("button", { name: "Hold to Record" })).toBeEnabled();
    expect(screen.getByRole("button", { name: "Show permissions details" })).toBeVisible();
  });

  it("refreshes permissions when the app regains focus and shows revoked access", async () => {
    let focusHandler: ((focused: boolean) => void) | undefined;

    vi.spyOn(desktop, "onCurrentWindowFocusChange").mockImplementation(async (handler) => {
      focusHandler = handler;
      return () => undefined;
    });
    vi.spyOn(desktop, "getPermissionStatus")
      .mockResolvedValueOnce(defaultPermissions)
      .mockResolvedValueOnce({
        microphone: "granted",
        accessibility: "denied",
      });

    render(<App />);

    expect(await screen.findByRole("button", { name: "Hold to Record" })).toBeEnabled();

    act(() => {
      focusHandler?.(true);
    });

    await waitFor(() => {
      expect(screen.getByText("Permissions required to continue")).toBeVisible();
    });
    expect(screen.getByRole("button", { name: "Hold to Record" })).toBeDisabled();
  });

  it("does not show the HUD when the setting is disabled", async () => {
    vi.spyOn(desktop, "getSettings").mockResolvedValue({
      ...defaultSettings,
      showHud: false,
    });

    render(<App />);

    const holdButton = await screen.findByRole("button", {
      name: "Hold to Record",
    });

    fireEvent.mouseDown(holdButton);
    fireEvent.mouseUp(await screen.findByRole("button", { name: "Release to Transcribe" }));

    await waitFor(() => {
      expect(screen.getByText("Inserted")).toBeVisible();
    });

    expect(desktop.showHud).not.toHaveBeenCalled();
    expect(desktop.setHudPhase).not.toHaveBeenCalled();
    expect(desktop.hideHud).toHaveBeenCalled();
  });

});
