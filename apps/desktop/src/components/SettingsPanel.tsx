import type { ChangeEvent } from "react";
import {
  DICTATION_MODES,
  RECORDING_INTERACTION_MODES,
  type Settings,
} from "@tator/shared-types";

interface SettingsPanelProps {
  settings: Settings;
  onChange: (settings: Settings) => void;
}

export function SettingsPanel({ settings, onChange }: SettingsPanelProps) {
  function update<K extends keyof Settings>(key: K, value: Settings[K]) {
    onChange({
      ...settings,
      [key]: value,
    });
  }

  return (
    <section className="panel">
      <div className="panel-header">
        <h2>Settings</h2>
        <span>Stored locally in SQLite</span>
      </div>
      <label className="field">
        <span>Global shortcut</span>
        <input
          onChange={(event: ChangeEvent<HTMLInputElement>) =>
            update("hotkey", event.target.value)
          }
          type="text"
          value={settings.hotkey}
        />
      </label>
      <label className="field">
        <span>Recording trigger</span>
        <select
          onChange={(event: ChangeEvent<HTMLSelectElement>) =>
            update("interactionMode", event.target.value as Settings["interactionMode"])
          }
          value={settings.interactionMode}
        >
          {RECORDING_INTERACTION_MODES.map((mode) => (
            <option key={mode} value={mode}>
              {mode === "hold" ? "Hold to record" : "Toggle on/off"}
            </option>
          ))}
        </select>
      </label>
      <label className="field">
        <span>Preferred mode</span>
        <select
          onChange={(event: ChangeEvent<HTMLSelectElement>) =>
            update("preferredMode", event.target.value as Settings["preferredMode"])
          }
          value={settings.preferredMode}
        >
          {DICTATION_MODES.map((mode) => (
            <option key={mode} value={mode}>
              {mode}
            </option>
          ))}
        </select>
      </label>
      <label className="toggle">
        <input
          checked={settings.autoPaste}
          onChange={(event: ChangeEvent<HTMLInputElement>) =>
            update("autoPaste", event.target.checked)
          }
          type="checkbox"
        />
        <span>Paste automatically after cleanup</span>
      </label>
      <label className="toggle">
        <input
          checked={settings.pauseBackgroundAudio}
          onChange={(event: ChangeEvent<HTMLInputElement>) =>
            update("pauseBackgroundAudio", event.target.checked)
          }
          type="checkbox"
        />
        <span>Pause background audio during recording</span>
      </label>
      <label className="field">
        <span>Recording cue volume ({settings.recordingCueVolume}%)</span>
        <input
          min="0"
          max="100"
          onChange={(event: ChangeEvent<HTMLInputElement>) =>
            update("recordingCueVolume", event.target.valueAsNumber)
          }
          step="1"
          type="range"
          value={settings.recordingCueVolume}
        />
      </label>
      <label className="toggle">
        <input
          checked={settings.restoreClipboard}
          onChange={(event: ChangeEvent<HTMLInputElement>) =>
            update("restoreClipboard", event.target.checked)
          }
          type="checkbox"
        />
        <span>Restore clipboard after paste</span>
      </label>
      <label className="toggle">
        <input
          checked={settings.saveHistory}
          onChange={(event: ChangeEvent<HTMLInputElement>) =>
            update("saveHistory", event.target.checked)
          }
          type="checkbox"
        />
        <span>Save recent dictation history</span>
      </label>
    </section>
  );
}
