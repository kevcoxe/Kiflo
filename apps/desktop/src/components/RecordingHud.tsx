import { useEffect, useState } from "react";
import {
  getHudState,
  onHudMeter,
  onHudStateChange,
  type HudPhase,
  type HudState,
} from "@/lib/desktop";

const HIDDEN_HUD_STATE: HudState = {
  visible: false,
  phase: "hidden",
};

function hudLabel(phase: HudPhase) {
  switch (phase) {
    case "recording":
      return "Recording";
    case "processing":
      return "Processing";
    case "error":
      return "Unable to finish";
    case "hidden":
      return "";
  }
}

export function RecordingHud() {
  const [hudState, setHudState] = useState<HudState>(HIDDEN_HUD_STATE);
  const [level, setLevel] = useState(0);

  useEffect(() => {
    let disposed = false;
    let removeStateListener: () => void = () => undefined;
    let removeMeterListener: () => void = () => undefined;

    async function bootstrap() {
      try {
        const [initialState, stateListener, meterListener] = await Promise.all([
          getHudState().catch(() => HIDDEN_HUD_STATE),
          onHudStateChange((nextState) => {
            if (disposed) {
              return;
            }

            setHudState(nextState);
            if (nextState.phase !== "recording") {
              setLevel(0);
            }
          }),
          onHudMeter(({ level: nextLevel }) => {
            if (disposed) {
              return;
            }

            setLevel(nextLevel);
          }),
        ]);

        if (disposed) {
          stateListener();
          meterListener();
          return;
        }

        removeStateListener = stateListener;
        removeMeterListener = meterListener;
        setHudState(initialState);
        if (initialState.phase !== "recording") {
          setLevel(0);
        }
      } catch {
        if (!disposed) {
          setHudState(HIDDEN_HUD_STATE);
          setLevel(0);
        }
      }
    }

    void bootstrap();

    return () => {
      disposed = true;
      removeStateListener();
      removeMeterListener();
    };
  }, []);

  if (!hudState.visible || hudState.phase === "hidden") {
    return null;
  }

  return (
    <main
      aria-live="polite"
      className={`hud-shell hud-phase-${hudState.phase}`}
      role="status"
    >
      <section className="hud-card">
        <div className="hud-badge" aria-hidden="true">
          {hudState.phase === "recording" ? (
            <span className="hud-mic-dot" />
          ) : hudState.phase === "processing" ? (
            <span className="hud-spinner-ring" />
          ) : (
            <span className="hud-error-mark">!</span>
          )}
        </div>
        <div className="hud-copy">
          <p className="hud-title">{hudLabel(hudState.phase)}</p>
          {hudState.phase === "recording" ? (
            <div aria-hidden="true" className="hud-meter">
              {[0, 1, 2, 3].map((bar) => {
                const intensity = Math.max(
                  0.18,
                  Math.min(1, level * (1 - bar * 0.12) + 0.2),
                );
                return (
                  <span
                    className="hud-meter-bar"
                    key={bar}
                    style={{ transform: `scaleY(${intensity})` }}
                  />
                );
              })}
            </div>
          ) : hudState.phase === "processing" ? (
            <>
              <p className="hud-subtitle">Cleaning up your dictation...</p>
              <div aria-hidden="true" className="hud-dots">
                <span />
                <span />
                <span />
              </div>
            </>
          ) : (
            <p className="hud-subtitle">Dictation hit a snag.</p>
          )}
        </div>
      </section>
    </main>
  );
}
