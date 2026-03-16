import { DICTATION_MODES } from "@tator/shared-types";
import type { KeyboardEvent, PointerEvent } from "react";
import { useEffect, useState } from "react";
import { Loader2 } from "lucide-react";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";
import { cn } from "@/lib/utils";
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

function statusBadgeVariant(status: ReturnType<typeof useDictationController>["status"]) {
  if (status === "recording" || status === "error") return "destructive" as const;
  return "secondary" as const;
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

  useEffect(() => {
    const html = document.documentElement;
    html.classList.remove("theme-light", "theme-dark");
    if (controller.settings.theme === "light") html.classList.add("theme-light");
    else if (controller.settings.theme === "dark") html.classList.add("theme-dark");
  }, [controller.settings.theme]);

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
    <main className="flex flex-col gap-4 mx-auto max-w-[920px] p-5">
      <Card>
        <CardContent className="pt-4 flex flex-wrap items-center gap-3">
          <Badge
            variant={statusBadgeVariant(controller.status)}
            className={cn(
              controller.status === "idle" || controller.status === "success"
                ? "bg-green-900/40 text-green-300 border-green-800/40"
                : undefined,
            )}
          >
            {statusLabel(controller.status)}
          </Badge>
          <div className="flex flex-wrap gap-3 ml-auto">
            <Button
              disabled={recordButtonDisabled}
              variant={controller.status === "processing" ? "secondary" : "default"}
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
                  <Loader2 className="animate-spin" />
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
            </Button>
            <select
              className={cn(
                "flex h-10 rounded-md border border-input bg-background px-3 py-2 text-sm",
                "ring-offset-background focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2",
              )}
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
        </CardContent>
      </Card>

      <Card>
        <CardContent className="pt-4">
          <div className="flex items-baseline justify-between gap-4 mb-3">
            <h2 className="text-base font-semibold leading-none tracking-tight m-0">Permissions</h2>
            {permissionsReady ? (
              <button
                aria-controls="permissions-panel-body"
                aria-expanded={!permissionsCollapsed}
                className="text-sm text-muted-foreground hover:text-foreground border border-border rounded-full bg-white/[0.06] px-3 py-1.5"
                onClick={() => setPermissionsExpanded((current) => !current)}
                type="button"
              >
                {permissionsCollapsed ? "Show permissions details" : "Hide permissions details"}
              </button>
            ) : null}
          </div>
          {permissionsCollapsed ? (
            <p className="text-muted-foreground m-0">Required permissions are granted.</p>
          ) : (
            <div id="permissions-panel-body">
              <dl className="grid gap-3 m-0">
                <div>
                  <dt className="text-muted-foreground text-sm">Microphone</dt>
                  <dd className="mt-0.5 font-bold m-0">{controller.permissions.microphone}</dd>
                </div>
                <div>
                  <dt className="text-muted-foreground text-sm">Accessibility</dt>
                  <dd className="mt-0.5 font-bold m-0">{controller.permissions.accessibility}</dd>
                </div>
              </dl>
              {!permissionsReady ? (
                <Alert className="mt-4">
                  <div className="grid grid-cols-[minmax(0,1fr)_auto] items-start gap-4">
                    <div>
                      <AlertTitle>Permissions required to continue</AlertTitle>
                      <AlertDescription>
                        Grant the required system permissions before using dictation.
                        Open the relevant system settings pages, then refresh once access has
                        been granted.
                      </AlertDescription>
                    </div>
                    <div className="grid gap-3 items-start">
                      {microphoneGranted ? null : (
                        <Button
                          onClick={() => void controller.openMicrophoneSystemSettings()}
                          type="button"
                          className="w-full"
                        >
                          Open Microphone Settings
                        </Button>
                      )}
                      {accessibilityGranted ? null : (
                        <Button
                          onClick={() => void controller.openAccessibilitySystemSettings()}
                          type="button"
                          className="w-full"
                        >
                          Open Accessibility Settings
                        </Button>
                      )}
                      <Button
                        variant="outline"
                        onClick={() => void controller.refreshPermissions()}
                        type="button"
                        className="w-full"
                      >
                        Refresh Permissions
                      </Button>
                    </div>
                  </div>
                </Alert>
              ) : null}
            </div>
          )}
        </CardContent>
      </Card>

      {controller.error ? (
        <Alert variant="destructive">
          <AlertDescription>{controller.error}</AlertDescription>
        </Alert>
      ) : null}

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
