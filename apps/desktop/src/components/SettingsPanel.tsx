import type { ChangeEvent, MutableRefObject } from "react";
import { useEffect, useRef, useState } from "react";
import {
  DICTATION_MODES,
  RECORDING_INTERACTION_MODES,
  SPEECH_MODELS,
  TYPING_STYLES,
  type ModelStatus,
  type SpeechModel,
  type Settings,
} from "@tator/shared-types";

interface SettingsPanelProps {
  settings: Settings;
  modelStatus: ModelStatus;
  onChange: (settings: Settings) => void;
  onDownloadModel: (model: SpeechModel) => void;
}

interface DictionaryRow {
  id: number;
  source: string;
  replacement: string;
}

const DEFAULT_RECORDING_CUE_VOLUME = 75;

export function SettingsPanel({
  settings,
  modelStatus,
  onChange,
  onDownloadModel,
}: SettingsPanelProps) {
  const previousCueVolumeRef = useRef(
    settings.recordingCueVolume > 0
      ? settings.recordingCueVolume
      : DEFAULT_RECORDING_CUE_VOLUME,
  );
  const nextDictionaryRowId = useRef(0);
  const syncedDictionaryRef = useRef(settings.customDictionary);
  const [dictionaryRows, setDictionaryRows] = useState<DictionaryRow[]>(() =>
    parseCustomDictionary(settings.customDictionary, () =>
      createDictionaryRow(nextDictionaryRowId),
    ),
  );

  if (settings.recordingCueVolume > 0) {
    previousCueVolumeRef.current = settings.recordingCueVolume;
  }

  useEffect(() => {
    if (settings.customDictionary === syncedDictionaryRef.current) {
      return;
    }

    syncedDictionaryRef.current = settings.customDictionary;
    setDictionaryRows(
      parseCustomDictionary(settings.customDictionary, () =>
        createDictionaryRow(nextDictionaryRowId),
      ),
    );
  }, [settings.customDictionary]);

  function update<K extends keyof Settings>(key: K, value: Settings[K]) {
    onChange({
      ...settings,
      [key]: value,
    });
  }

  function updateDictionary(nextRows: DictionaryRow[]) {
    const customDictionary = serializeCustomDictionary(nextRows);
    syncedDictionaryRef.current = customDictionary;
    setDictionaryRows(nextRows);
    update("customDictionary", customDictionary);
  }

  function addDictionaryRow() {
    setDictionaryRows((current) => [
      ...current,
      createDictionaryRow(nextDictionaryRowId),
    ]);
  }

  function updateDictionaryRow(
    rowId: number,
    key: "source" | "replacement",
    value: string,
  ) {
    const nextRows = dictionaryRows.map((row) =>
      row.id === rowId ? { ...row, [key]: value } : row,
    );
    updateDictionary(nextRows);
  }

  function removeDictionaryRow(rowId: number) {
    updateDictionary(dictionaryRows.filter((row) => row.id !== rowId));
  }

  const selectableSpeechModels = SPEECH_MODELS.filter(
    (model) =>
      modelStatus.installedModels.includes(model) || settings.speechModel === model,
  );
  const smallModelInstalled = modelStatus.installedModels.includes("small_en");
  const downloadingSmallModel =
    modelStatus.downloading && modelStatus.downloadTarget === "small_en";

  return (
    <section className="panel">
      <div className="panel-header">
        <h2>Settings</h2>
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
      <label className="field">
        <span>Typing style</span>
        <p className="field-help muted">
          Go formal in docs, very casual in messages, or excited in upbeat notes.
        </p>
        <select
          onChange={(event: ChangeEvent<HTMLSelectElement>) =>
            update("typingStyle", event.target.value as Settings["typingStyle"])
          }
          value={settings.typingStyle}
        >
          {TYPING_STYLES.map((style) => (
            <option key={style} value={style}>
              {labelForTypingStyle(style)}
            </option>
          ))}
        </select>
      </label>
      <div className="field">
        <span>Speech model</span>
        <p className="field-help muted">
          Base English is bundled with the app. Small English is more accurate, but
          it adds a larger local download and uses more disk space.
        </p>
        <select
          aria-label="Speech model"
          disabled={modelStatus.downloading}
          onChange={(event: ChangeEvent<HTMLSelectElement>) =>
            update("speechModel", event.target.value as Settings["speechModel"])
          }
          value={settings.speechModel}
        >
          {selectableSpeechModels.map((model) => (
            <option key={model} value={model}>
              {labelForSpeechModel(model, modelStatus.bundledModel)}
            </option>
          ))}
        </select>
        {!smallModelInstalled ? (
          <>
            <p className="field-help muted">
              Download Small English for higher accuracy. It adds about 488 MB to app
              data.
            </p>
            <div className="dictionary-actions">
              <button
                className="secondary-button"
                disabled={modelStatus.downloading}
                onClick={() => onDownloadModel("small_en")}
                type="button"
              >
                {downloadingSmallModel
                  ? "Downloading small model..."
                  : "Download small model (~488 MB)"}
              </button>
            </div>
          </>
        ) : null}
      </div>
      <div className="field">
        <span id="custom-dictionary-label">Custom dictionary</span>
        <p className="field-help muted">
          Each row replaces whole words or phrases only, case-insensitively.
        </p>
        <div className="dictionary-table-wrap">
          <table aria-labelledby="custom-dictionary-label" className="dictionary-table">
            <thead>
            {dictionaryRows.length > 0 ? (
              <tr>
                <th scope="col">Heard</th>
                <th scope="col">Replace with</th>
                <th aria-label="Actions" scope="col" />
              </tr>
            ) : null}
            </thead>
            <tbody>
              {dictionaryRows.length > 0 ? (
                dictionaryRows.map((row, index) => (
                  <tr key={row.id}>
                    <td>
                      <input
                        aria-label={`Dictionary source ${index + 1}`}
                        onChange={(event: ChangeEvent<HTMLInputElement>) =>
                          updateDictionaryRow(row.id, "source", event.target.value)
                        }
                        placeholder="Rebecca"
                        spellCheck={false}
                        type="text"
                        value={row.source}
                      />
                    </td>
                    <td>
                      <input
                        aria-label={`Dictionary replacement ${index + 1}`}
                        onChange={(event: ChangeEvent<HTMLInputElement>) =>
                          updateDictionaryRow(row.id, "replacement", event.target.value)
                        }
                        placeholder="Rebekah"
                        spellCheck={false}
                        type="text"
                        value={row.replacement}
                      />
                    </td>
                    <td className="dictionary-row-actions">
                      <button
                        className="secondary-button"
                        onClick={() => removeDictionaryRow(row.id)}
                        type="button"
                      >
                        Remove
                      </button>
                    </td>
                  </tr>
                ))
              ) : (
                <tr>
                  <td className="dictionary-empty muted" colSpan={3}>
                    No custom replacements yet.
                  </td>
                </tr>
              )}
            </tbody>
          </table>
        </div>
        <div className="dictionary-actions">
          <button className="secondary-button" onClick={addDictionaryRow} type="button">
            Add word
          </button>
        </div>
      </div>
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
          checked={settings.autoBacktrack}
          onChange={(event: ChangeEvent<HTMLInputElement>) =>
            update("autoBacktrack", event.target.checked)
          }
          type="checkbox"
        />
        <span>Enable Auto Backtrack for spoken corrections</span>
      </label>
      <label className="toggle">
        <input
          checked={settings.spokenPressEnterCommand}
          onChange={(event: ChangeEvent<HTMLInputElement>) =>
            update("spokenPressEnterCommand", event.target.checked)
          }
          type="checkbox"
        />
        <span>Enable spoken "press enter" at the end of a recording</span>
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
      <label className="toggle">
        <input
          checked={settings.warmModelOnLaunch}
          onChange={(event: ChangeEvent<HTMLInputElement>) =>
            update("warmModelOnLaunch", event.target.checked)
          }
          type="checkbox"
        />
        <span>Warm speech model on app launch</span>
      </label>
      <label className="toggle">
        <input
          onChange={(event: ChangeEvent<HTMLInputElement>) =>
            update(
              "recordingCueVolume",
              event.target.checked ? previousCueVolumeRef.current : 0,
            )
          }
          checked={settings.recordingCueVolume > 0}
          type="checkbox"
        />
        <span>Play recording cue</span>
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
      <label className="toggle">
        <input
          checked={settings.hideDockIcon}
          onChange={(event: ChangeEvent<HTMLInputElement>) =>
            update("hideDockIcon", event.target.checked)
          }
          type="checkbox"
        />
        <span>Hide Dock icon after closing the window</span>
      </label>
      <label className="toggle">
        <input
          checked={settings.showHud}
          onChange={(event: ChangeEvent<HTMLInputElement>) =>
            update("showHud", event.target.checked)
          }
          type="checkbox"
        />
        <span>Show recording HUD overlay</span>
      </label>
    </section>
  );
}

function labelForSpeechModel(model: SpeechModel, bundledModel: SpeechModel): string {
  const baseLabel = model === "base_en" ? "Base English" : "Small English";
  return model === bundledModel ? `${baseLabel} (bundled)` : baseLabel;
}

function createDictionaryRow(nextDictionaryRowId: MutableRefObject<number>): DictionaryRow {
  const row = {
    id: nextDictionaryRowId.current,
    source: "",
    replacement: "",
  };
  nextDictionaryRowId.current += 1;
  return row;
}

function parseCustomDictionary(
  customDictionary: string,
  createRow: () => DictionaryRow,
): DictionaryRow[] {
  return customDictionary
    .split("\n")
    .map((line) => parseDictionaryLine(line, createRow))
    .filter((row): row is DictionaryRow => row !== null);
}

function parseDictionaryLine(
  line: string,
  createRow: () => DictionaryRow,
): DictionaryRow | null {
  const trimmed = line.trim();
  if (!trimmed) {
    return null;
  }

  const entry = trimmed.match(/^(.*?)(?:=>|->|=)(.*)$/);
  if (!entry) {
    return null;
  }

  const source = entry[1]?.trim() ?? "";
  const replacement = entry[2]?.trim() ?? "";
  if (!source || !replacement) {
    return null;
  }

  return {
    ...createRow(),
    source,
    replacement,
  };
}

function serializeCustomDictionary(rows: DictionaryRow[]): string {
  return rows
    .map((row) => ({
      source: row.source.trim(),
      replacement: row.replacement.trim(),
    }))
    .filter((row) => row.source && row.replacement)
    .map((row) => `${row.source} => ${row.replacement}`)
    .join("\n");
}

function labelForTypingStyle(style: Settings["typingStyle"]): string {
  switch (style) {
    case "very_casual":
      return "Very casual";
    case "formal":
      return "Formal";
    case "excited":
      return "Excited";
  }
}
