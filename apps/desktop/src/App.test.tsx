import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import App from "./App";
import * as desktop from "@/lib/desktop";
import { ensureTray } from "@/lib/tray";

vi.mock("@/lib/tray", () => ({
  ensureTray: vi.fn().mockResolvedValue(undefined),
}));

const defaultSettings = {
  hotkey: "Option+Space",
  apiBaseUrl: "http://127.0.0.1:8000",
  autoPaste: true,
  pauseBackgroundAudio: true,
  restoreClipboard: true,
  interactionMode: "hold" as const,
  preferredMode: "default" as const,
  saveHistory: true,
};

const defaultPermissions = {
  microphone: "granted" as const,
  accessibility: "granted" as const,
};

function mockSuccessfulBootstrap() {
  vi.spyOn(desktop, "getSettings").mockResolvedValue(defaultSettings);
  vi.spyOn(desktop, "getPermissionStatus").mockResolvedValue(defaultPermissions);
  vi.spyOn(desktop, "listHistory").mockResolvedValue([]);
  vi.spyOn(desktop, "updateSettings").mockImplementation(async (settings) => settings);
  vi.spyOn(desktop, "registerGlobalShortcut").mockResolvedValue(undefined);
  vi.spyOn(desktop, "unregisterGlobalShortcut").mockResolvedValue(undefined);
  vi.spyOn(desktop, "unregisterAllGlobalShortcuts").mockResolvedValue(undefined);
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
  vi.spyOn(desktop, "createHistoryRecord").mockImplementation(async (record) => ({
    id: "history-1",
    createdAt: "2026-03-11T00:00:00Z",
    ...record,
    appName: record.appName ?? null,
  }));
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
    vi.stubGlobal(
      "fetch",
      vi
        .fn()
        .mockResolvedValueOnce(
          new Response(
            JSON.stringify({
              raw_text: "hey can you send the update tomorrow",
              language: "en",
              duration_ms: 450,
              provider: "mock",
            }),
            { status: 200 },
          ),
        )
        .mockResolvedValueOnce(
          new Response(
            JSON.stringify({
              clean_text: "Hey, can you send the update tomorrow?",
              provider: "mock",
            }),
            { status: 200 },
          ),
        ),
    );

    render(<App />);

    const holdButton = await screen.findByRole("button", {
      name: "Hold to Record",
    });

    fireEvent.mouseDown(holdButton);
    expect(await screen.findByRole("button", { name: "Release to Transcribe" })).toBeVisible();
    expect(desktop.startRecording).toHaveBeenCalledWith(true);

    fireEvent.mouseUp(screen.getByRole("button", { name: "Release to Transcribe" }));

    await waitFor(() => {
      expect(screen.getByText("Inserted")).toBeVisible();
    });

    expect(
      screen.getAllByText("Hey, can you send the update tomorrow?").length,
    ).toBeGreaterThan(0);
  });

  it("shows an error banner when the API call fails", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValue(new Response("upstream failed", { status: 500 })),
    );

    render(<App />);

    const holdButton = await screen.findByRole("button", { name: "Hold to Record" });
    fireEvent.mouseDown(holdButton);
    fireEvent.mouseUp(await screen.findByRole("button", { name: "Release to Transcribe" }));

    expect(await screen.findByText("upstream failed")).toBeVisible();
  });

  it("opens the fallback panel when paste fails", async () => {
    vi.stubGlobal(
      "fetch",
      vi
        .fn()
        .mockResolvedValueOnce(
          new Response(
            JSON.stringify({
              raw_text: "running five minutes late be there soon",
              language: "en",
              duration_ms: 450,
              provider: "mock",
            }),
            { status: 200 },
          ),
        )
        .mockResolvedValueOnce(
          new Response(
            JSON.stringify({
              clean_text: "Running five minutes late. Be there soon.",
              provider: "mock",
            }),
            { status: 200 },
          ),
        ),
    );
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

  it("creates the tray once the app boots", async () => {
    vi.stubGlobal("fetch", vi.fn());

    render(<App />);

    await waitFor(() => {
      expect(ensureTray).toHaveBeenCalledTimes(1);
    });
  });

  it("lets users disable background audio pausing", async () => {
    vi.stubGlobal("fetch", vi.fn());

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

  it("supports toggle mode when configured in settings", async () => {
    vi.spyOn(desktop, "getSettings").mockResolvedValue({
      ...defaultSettings,
      interactionMode: "toggle",
    });
    vi.stubGlobal(
      "fetch",
      vi
        .fn()
        .mockResolvedValueOnce(
          new Response(
            JSON.stringify({
              raw_text: "follow up tomorrow morning",
              language: "en",
              duration_ms: 450,
              provider: "mock",
            }),
            { status: 200 },
          ),
        )
        .mockResolvedValueOnce(
          new Response(
            JSON.stringify({
              clean_text: "Follow up tomorrow morning.",
              provider: "mock",
            }),
            { status: 200 },
          ),
        ),
    );

    render(<App />);

    const toggleButton = await screen.findByRole("button", { name: "Start Recording" });
    fireEvent.click(toggleButton);
    expect(await screen.findByRole("button", { name: "Stop Recording" })).toBeVisible();

    fireEvent.click(screen.getByRole("button", { name: "Stop Recording" }));

    await waitFor(() => {
      expect(screen.getByText("Inserted")).toBeVisible();
    });
  });
});
