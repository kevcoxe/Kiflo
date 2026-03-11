import { DICTATION_MODES } from "@tator/shared-types";
import { FallbackPanel } from "@/components/FallbackPanel";
import { HistoryList } from "@/components/HistoryList";
import { SettingsPanel } from "@/components/SettingsPanel";
import { useDictationController } from "@/hooks/useDictationController";

function statusLabel(status: ReturnType<typeof useDictationController>["status"]) {
  switch (status) {
    case "booting":
      return "Booting";
    case "idle":
      return "Ready";
    case "recording":
      return "Recording";
    case "processing":
      return "Processing";
    case "success":
      return "Inserted";
    case "error":
      return "Attention Needed";
  }
}

export default function App() {
  const controller = useDictationController();

  return (
    <main className="app-shell">
      <section className="hero panel">
        <div>
          <p className="eyebrow">macOS dictation MVP</p>
          <h1>Tator</h1>
          <p className="lead">
            Toggle dictation with {controller.settings.hotkey}, clean the text
            through the API, then paste it back into the frontmost app.
          </p>
        </div>
        <div className={`status-badge status-${controller.status}`}>
          {statusLabel(controller.status)}
        </div>
        <div className="button-row">
          <button
            className="primary-button"
            disabled={!controller.initialized || controller.status === "processing"}
            onClick={() => void controller.toggleRecording()}
            type="button"
          >
            {controller.status === "recording" ? "Stop Recording" : "Start Recording"}
          </button>
          <select
            className="mode-select"
            onChange={(event) =>
              controller.setMode(event.target.value as (typeof DICTATION_MODES)[number])
            }
            value={controller.mode}
          >
            {DICTATION_MODES.map((mode) => (
              <option key={mode} value={mode}>
                {mode}
              </option>
            ))}
          </select>
        </div>
      </section>

      <section className="grid-layout">
        <section className="panel">
          <div className="panel-header">
            <h2>Permissions</h2>
            <span>Best-effort status</span>
          </div>
          <dl className="permission-grid">
            <div>
              <dt>Microphone</dt>
              <dd>{controller.permissions.microphone}</dd>
            </div>
            <div>
              <dt>Accessibility</dt>
              <dd>{controller.permissions.accessibility}</dd>
            </div>
          </dl>
        </section>

        <section className="panel">
          <div className="panel-header">
            <h2>Last output</h2>
            <span>Raw and cleaned text</span>
          </div>
          <div className="result-block">
            <strong>Raw</strong>
            <p>{controller.lastRawText ?? "Nothing captured yet."}</p>
          </div>
          <div className="result-block">
            <strong>Clean</strong>
            <p>{controller.lastCleanText ?? "Cleanup output will appear here."}</p>
          </div>
        </section>
      </section>

      {controller.error ? <p className="error-banner">{controller.error}</p> : null}

      {controller.fallbackText ? (
        <FallbackPanel
          onCopy={() => void controller.copyFallbackText()}
          onRetry={() => void controller.retryPaste()}
          text={controller.fallbackText}
        />
      ) : null}

      <SettingsPanel
        onChange={(nextSettings) => void controller.saveSettings(nextSettings)}
        settings={controller.settings}
      />
      <HistoryList history={controller.history} />
    </main>
  );
}
