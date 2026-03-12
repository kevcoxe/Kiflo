import { act, render, screen, waitFor } from "@testing-library/react";
import { RecordingHud } from "./RecordingHud";
import * as desktop from "@/lib/desktop";

describe("RecordingHud", () => {
  beforeEach(() => {
    vi.restoreAllMocks();
  });

  it("renders the recording state and reacts to level updates", async () => {
    let stateHandler: ((state: desktop.HudState) => void) | null = null;
    let meterHandler: ((payload: desktop.HudMeterPayload) => void) | null = null;

    vi.spyOn(desktop, "getHudState").mockResolvedValue({
      visible: true,
      phase: "recording",
    });
    vi.spyOn(desktop, "onHudStateChange").mockImplementation(async (handler) => {
      stateHandler = handler;
      return () => undefined;
    });
    vi.spyOn(desktop, "onHudMeter").mockImplementation(async (handler) => {
      meterHandler = handler;
      return () => undefined;
    });

    render(<RecordingHud />);

    expect(await screen.findByText("Recording")).toBeVisible();

    await act(async () => {
      meterHandler?.({ level: 0.75 });
    });

    await waitFor(() => {
      const bars = document.querySelectorAll(".hud-meter-bar");
      expect(bars).toHaveLength(4);
      expect((bars[0] as HTMLElement).style.transform).toContain("scaleY");
    });

    await act(async () => {
      stateHandler?.({ visible: true, phase: "processing" });
    });

    expect(await screen.findByText("Processing")).toBeVisible();
    expect(screen.getByText("Cleaning up your dictation...")).toBeVisible();
    expect(document.querySelector(".hud-spinner-ring")).toBeTruthy();
  });

  it("hides itself when the HUD state becomes hidden", async () => {
    let stateHandler: ((state: desktop.HudState) => void) | null = null;

    vi.spyOn(desktop, "getHudState").mockResolvedValue({
      visible: true,
      phase: "recording",
    });
    vi.spyOn(desktop, "onHudStateChange").mockImplementation(async (handler) => {
      stateHandler = handler;
      return () => undefined;
    });
    vi.spyOn(desktop, "onHudMeter").mockResolvedValue(() => undefined);

    const { container } = render(<RecordingHud />);

    expect(await screen.findByText("Recording")).toBeVisible();

    await act(async () => {
      stateHandler?.({ visible: false, phase: "hidden" });
    });

    await waitFor(() => {
      expect(container).toBeEmptyDOMElement();
    });
  });
});
