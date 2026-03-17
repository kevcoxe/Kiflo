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
  THEMES,
} from "@tator/shared-types";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Switch } from "@/components/ui/switch";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import { cn } from "@/lib/utils";

interface SettingsPanelProps {
  settings: Settings;
  modelStatus: ModelStatus;
  onChange: (settings: Settings) => void;
}

interface DictionaryRow {
  id: number;
  source: string;
  replacement: string;
}

const DEFAULT_RECORDING_CUE_VOLUME = 75;

const selectClass = cn(
  "flex h-10 w-full rounded-md border border-input bg-background px-3 py-2 text-sm",
  "ring-offset-background focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2",
  "disabled:cursor-not-allowed disabled:opacity-50",
);

export function SettingsPanel({
  settings,
  modelStatus,
  onChange,
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

  return (
    <Card>
      <CardHeader className="pb-3">
        <CardTitle className="text-base">Settings</CardTitle>
      </CardHeader>
      <CardContent className="space-y-4">
        <div className="grid gap-1.5">
          <Label htmlFor="appearance-select">Appearance</Label>
          <select
            id="appearance-select"
            className={selectClass}
            onChange={(event: ChangeEvent<HTMLSelectElement>) =>
              update("theme", event.target.value as Settings["theme"])
            }
            value={settings.theme}
          >
            {THEMES.map((t) => (
              <option key={t} value={t}>
                {t === "system" ? "System" : t === "light" ? "Light" : "Dark"}
              </option>
            ))}
          </select>
        </div>

        <div className="grid gap-1.5">
          <Label htmlFor="hotkey-input">Global shortcut</Label>
          <Input
            id="hotkey-input"
            onChange={(event: ChangeEvent<HTMLInputElement>) =>
              update("hotkey", event.target.value)
            }
            type="text"
            value={settings.hotkey}
          />
        </div>

        <div className="grid gap-1.5">
          <Label htmlFor="interaction-mode-select">Recording trigger</Label>
          <select
            id="interaction-mode-select"
            className={selectClass}
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
        </div>

        <div className="grid gap-1.5">
          <Label htmlFor="preferred-mode-select">Preferred mode</Label>
          <select
            id="preferred-mode-select"
            className={selectClass}
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
        </div>

        <div className="grid gap-1.5">
          <Label htmlFor="typing-style-select">Typing style</Label>
          <p className="text-sm text-muted-foreground m-0">
            Go formal in docs, very casual in messages, or excited in upbeat notes.
          </p>
          <select
            id="typing-style-select"
            className={selectClass}
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
        </div>

        <div className="grid gap-1.5">
          <Label htmlFor="speech-model-select">Speech model</Label>
          <p className="text-sm text-muted-foreground m-0">
            Base English and Small English are bundled with the app. Small English is
            more accurate, but it uses more disk space and takes longer to warm up.
          </p>
          <select
            aria-label="Speech model"
            id="speech-model-select"
            className={selectClass}
            disabled={modelStatus.downloading}
            onChange={(event: ChangeEvent<HTMLSelectElement>) =>
              update("speechModel", event.target.value as Settings["speechModel"])
            }
            value={settings.speechModel}
          >
            {selectableSpeechModels.map((model) => (
              <option key={model} value={model}>
                {labelForSpeechModel(model, modelStatus.bundledModels)}
              </option>
            ))}
          </select>
        </div>

        <div className="grid gap-1.5">
          <Label id="custom-dictionary-label">Custom dictionary</Label>
          <p className="text-sm text-muted-foreground m-0">
            Each row replaces whole words or phrases only, case-insensitively.
          </p>
          <div className="overflow-x-auto">
            <Table aria-labelledby="custom-dictionary-label">
              <TableHeader>
                {dictionaryRows.length > 0 ? (
                  <TableRow>
                    <TableHead>Heard</TableHead>
                    <TableHead>Replace with</TableHead>
                    <TableHead aria-label="Actions" />
                  </TableRow>
                ) : null}
              </TableHeader>
              <TableBody>
                {dictionaryRows.length > 0 ? (
                  dictionaryRows.map((row, index) => (
                    <TableRow key={row.id}>
                      <TableCell>
                        <Input
                          aria-label={`Dictionary source ${index + 1}`}
                          onChange={(event: ChangeEvent<HTMLInputElement>) =>
                            updateDictionaryRow(row.id, "source", event.target.value)
                          }
                          placeholder="Rebecca"
                          spellCheck={false}
                          type="text"
                          value={row.source}
                        />
                      </TableCell>
                      <TableCell>
                        <Input
                          aria-label={`Dictionary replacement ${index + 1}`}
                          onChange={(event: ChangeEvent<HTMLInputElement>) =>
                            updateDictionaryRow(row.id, "replacement", event.target.value)
                          }
                          placeholder="Rebekah"
                          spellCheck={false}
                          type="text"
                          value={row.replacement}
                        />
                      </TableCell>
                      <TableCell className="w-px whitespace-nowrap">
                        <Button
                          variant="outline"
                          className="border-red-800/40 bg-red-950/20 text-red-300 hover:bg-red-950/40 hover:text-red-200"
                          onClick={() => removeDictionaryRow(row.id)}
                          type="button"
                        >
                          Remove
                        </Button>
                      </TableCell>
                    </TableRow>
                  ))
                ) : (
                  <TableRow>
                    <TableCell colSpan={3} className="text-muted-foreground py-3.5">
                      No custom replacements yet.
                    </TableCell>
                  </TableRow>
                )}
              </TableBody>
            </Table>
          </div>
          <div className="flex justify-start mt-1">
            <Button variant="outline" onClick={addDictionaryRow} type="button">
              Add word
            </Button>
          </div>
        </div>

        <ToggleSetting
          id="auto-paste"
          checked={settings.autoPaste}
          onChange={(checked) => update("autoPaste", checked)}
          label="Paste automatically after cleanup"
        />
        <ToggleSetting
          id="auto-backtrack"
          checked={settings.autoBacktrack}
          onChange={(checked) => update("autoBacktrack", checked)}
          label="Enable Auto Backtrack for spoken corrections"
        />
        <div className="flex flex-col gap-1">
          <ToggleSetting
            id="editor-file-mentions"
            checked={settings.editorFileMentionsEnabled}
            onChange={(checked) => update("editorFileMentionsEnabled", checked)}
            label="Enable editor file mentions"
          />
          <p className="text-sm text-muted-foreground m-0 pl-[52px]">
            In supported editor chat boxes, convert spoken file names into @mentions and
            confirm them with Tab.
          </p>
        </div>
        <div className="flex flex-col gap-1">
          <ToggleSetting
            id="force-editor-composer"
            checked={settings.forceEditorComposer}
            onChange={(checked) => update("forceEditorComposer", checked)}
            label="Force supported editors to behave like composer fields"
          />
          <p className="text-sm text-muted-foreground m-0 pl-[52px]">
            Use @mentions and Tab even when Tator cannot confirm the focused field is a
            chat/composer input.
          </p>
        </div>
        <ToggleSetting
          id="spoken-press-enter"
          checked={settings.spokenPressEnterCommand}
          onChange={(checked) => update("spokenPressEnterCommand", checked)}
          label='Enable spoken "press enter" at the end of a recording'
        />
        <ToggleSetting
          id="pause-background-audio"
          checked={settings.pauseBackgroundAudio}
          onChange={(checked) => update("pauseBackgroundAudio", checked)}
          label="Pause background audio during recording"
        />
        <ToggleSetting
          id="warm-model-on-launch"
          checked={settings.warmModelOnLaunch}
          onChange={(checked) => update("warmModelOnLaunch", checked)}
          label="Warm speech model on app launch"
        />
        <ToggleSetting
          id="recording-cue"
          checked={settings.recordingCueVolume > 0}
          onChange={(checked) =>
            update(
              "recordingCueVolume",
              checked ? previousCueVolumeRef.current : 0,
            )
          }
          label="Play recording cue"
        />
        <ToggleSetting
          id="restore-clipboard"
          checked={settings.restoreClipboard}
          onChange={(checked) => update("restoreClipboard", checked)}
          label="Restore clipboard after paste"
        />
        <ToggleSetting
          id="save-history"
          checked={settings.saveHistory}
          onChange={(checked) => update("saveHistory", checked)}
          label="Save recent dictation history"
        />
        <ToggleSetting
          id="hide-dock-icon"
          checked={settings.hideDockIcon}
          onChange={(checked) => update("hideDockIcon", checked)}
          label="Hide Dock icon after closing the window"
        />
        <ToggleSetting
          id="show-hud"
          checked={settings.showHud}
          onChange={(checked) => update("showHud", checked)}
          label="Show recording HUD overlay"
        />
      </CardContent>
    </Card>
  );
}

interface ToggleSettingProps {
  id: string;
  checked: boolean;
  onChange: (checked: boolean) => void;
  label: string;
}

function ToggleSetting({ id, checked, onChange, label }: ToggleSettingProps) {
  return (
    <div className="flex items-center gap-3">
      <Switch id={id} checked={checked} onCheckedChange={onChange} />
      <Label htmlFor={id}>{label}</Label>
    </div>
  );
}

function labelForSpeechModel(model: SpeechModel, bundledModels: SpeechModel[]): string {
  const baseLabel = model === "base_en" ? "Base English" : "Small English";
  return bundledModels.includes(model) ? `${baseLabel} (bundled)` : baseLabel;
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
