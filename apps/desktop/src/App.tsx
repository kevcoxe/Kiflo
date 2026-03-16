import { DICTATION_MODES } from "@tator/shared-types";
import type { KeyboardEvent, PointerEvent } from "react";
import { useEffect, useState } from "react";
import { FallbackPanel } from "@/components/FallbackPanel";
import { HistoryList } from "@/components/HistoryList";
import { MetricsDashboard } from "@/components/MetricsDashboard";
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
  const isHoldMode = controller.settings.interactionMode === "hold";
  const microphoneGranted = controller.permissions.microphone === "granted";
  const accessibilityGranted = controller.permissions.accessibility === "granted";
  const permissionsReady = microphoneGranted && accessibilityGranted;
  const [permissionsExpanded, setPermissionsExpanded] = useState(false);
  const permissionsCollapsed = permissionsReady && !permissionsExpanded;
  const recordButtonDisabled =
    !controller.initialized || controller.status === "processing" || !permissionsReady;

  useEffect(() => {
    if (!permissionsReady) {
      setPermissionsExpanded(false);
    }
  }, [permissionsReady]);

  function handlePressStart(event: PointerEvent<HTMLButtonElement>) {
    if (!isHoldMode) {
      return;
    }

    if (typeof event.currentTarget.setPointerCapture === "function") {
      try {
        event.currentTarget.setPointerCapture(event.pointerId);
      } catch {
        // Ignore pointer-capture failures and still start recording.
      }
    }

    void controller.startHoldRecording();
  }

  function handleMousePressStart() {
    if (!isHoldMode) {
      return;
    }

    void controller.startHoldRecording();
  }

  function handlePressStop(event: PointerEvent<HTMLButtonElement>) {
    if (!isHoldMode) {
      return;
    }

    if (
      typeof event.currentTarget.hasPointerCapture === "function" &&
      typeof event.currentTarget.releasePointerCapture === "function"
    ) {
      try {
        if (event.currentTarget.hasPointerCapture(event.pointerId)) {
          event.currentTarget.releasePointerCapture(event.pointerId);
        }
      } catch {
        // Ignore pointer-capture release failures.
      }
    }

    void controller.stopHoldRecording();
  }

  function handleMousePressStop() {
    if (!isHoldMode) {
      return;
    }

    void controller.stopHoldRecording();
  }

  function handleKeyDown(event: KeyboardEvent<HTMLButtonElement>) {
    if (!isHoldMode) {
      return;
    }

    if (event.repeat) {
      return;
    }

    if (event.key === " " || event.key === "Enter") {
      event.preventDefault();
      void controller.startHoldRecording();
    }
  }

  function handleKeyUp(event: KeyboardEvent<HTMLButtonElement>) {
    if (!isHoldMode) {
      return;
    }

    if (event.key === " " || event.key === "Enter") {
      event.preventDefault();
      void controller.stopHoldRecording();
    }
  }

  return (
    <main className="app-shell">
      <section className="panel control-bar">
        <div className={`status-badge status-${controller.status}`}>
          {statusLabel(controller.status)}
        </div>
        <div className="button-row">
          <button
            className={`primary-button ${
              controller.status === "processing" ? "is-processing" : ""
            }`}
            disabled={recordButtonDisabled}
            onClick={() => {
              if (!isHoldMode) {
                void controller.toggleRecording();
              }
            }}
            onKeyDown={handleKeyDown}
            onKeyUp={handleKeyUp}
            onMouseDown={handleMousePressStart}
            onMouseUp={handleMousePressStop}
            onPointerCancel={handlePressStop}
            onPointerDown={handlePressStart}
            onPointerUp={handlePressStop}
            type="button"
          >
            {controller.status === "processing" ? (
              <>
                <span aria-hidden="true" className="button-spinner" />
                <span>Processing...</span>
              </>
            ) : isHoldMode ? (
              controller.status === "recording"
                ? "Release to Transcribe"
                : "Hold to Record"
            ) : controller.status === "recording" ? (
              "Stop Recording"
            ) : (
              "Start Recording"
            )}
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

      <section className="panel">
        <div className="panel-header">
          <h2>Permissions</h2>
          {permissionsReady ? (
            <button
              aria-controls="permissions-panel-body"
              aria-expanded={!permissionsCollapsed}
              className="panel-toggle"
              onClick={() => setPermissionsExpanded((current) => !current)}
              type="button"
            >
              {permissionsCollapsed ? "Show permissions details" : "Hide permissions details"}
            </button>
          ) : null}
        </div>
        {permissionsCollapsed ? (
          <p className="muted permissions-summary">
            Required permissions are granted.
          </p>
        ) : (
          <div id="permissions-panel-body">
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
            {!permissionsReady ? (
              <div className="result-block permissions-callout">
                <div className="permissions-copy">
                  <strong>Permissions required to continue</strong>
                  <p>
                    Grant the required system permissions before using dictation.
                    Open the relevant system settings pages, then refresh once access has
                    been granted.
                  </p>
                </div>
                <div className="permissions-actions">
                  {microphoneGranted ? null : (
                    <button
                      className="primary-button"
                      onClick={() => void controller.openMicrophoneSystemSettings()}
                      type="button"
                    >
                      Open Microphone Settings
                    </button>
                  )}
                  {accessibilityGranted ? null : (
                    <button
                      className="primary-button"
                      onClick={() => void controller.openAccessibilitySystemSettings()}
                      type="button"
                    >
                      Open Accessibility Settings
                    </button>
                  )}
                  <button
                    className="mode-select"
                    onClick={() => void controller.refreshPermissions()}
                    type="button"
                  >
                    Refresh Permissions
                  </button>
                </div>
              </div>
            ) : null}
          </div>
        )}
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
        modelStatus={controller.modelStatus}
        settings={controller.settings}
      />
      <MetricsDashboard metrics={controller.historyMetrics} />
      <HistoryList history={controller.history} />
    </main>
  );
}
