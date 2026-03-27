import type { ChangeEvent } from "react";
import { useRef } from "react";
import {
  DICTATION_MODES,
  type InputDevice,
  RECORDING_INTERACTION_MODES,
  SPEECH_MODELS,
  TYPING_STYLES,
  type ModelStatus,
  type SpeechModel,
  type Settings,
  THEMES,
} from "@kiflo/shared-types";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Switch } from "@/components/ui/switch";
import { cn } from "@/lib/utils";

interface SettingsPanelProps {
  settings: Settings;
  inputDevices: InputDevice[];
  modelStatus: ModelStatus;
  onChange: (settings: Settings) => void;
}

const DEFAULT_RECORDING_CUE_VOLUME = 75;

const selectClass = cn(
  "flex h-10 w-full rounded-md border border-input bg-background px-3 py-2 text-sm",
  "ring-offset-background focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2",
  "disabled:cursor-not-allowed disabled:opacity-50",
);

export function SettingsPanel({
  settings,
  inputDevices,
  modelStatus,
  onChange,
}: SettingsPanelProps) {
  const previousCueVolumeRef = useRef(
    settings.recordingCueVolume > 0
      ? settings.recordingCueVolume
      : DEFAULT_RECORDING_CUE_VOLUME,
  );

  if (settings.recordingCueVolume > 0) {
    previousCueVolumeRef.current = settings.recordingCueVolume;
  }

  function update<K extends keyof Settings>(key: K, value: Settings[K]) {
    onChange({
      ...settings,
      [key]: value,
    });
  }

  const selectableSpeechModels = SPEECH_MODELS.filter(
    (model) =>
      modelStatus.installedModels.includes(model) || settings.speechModel === model,
  );

  return (
    <div className="space-y-6 p-4">
      <SettingsSection title="Appearance">
        <SelectSetting label="Theme">
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
        </SelectSetting>
      </SettingsSection>

      <SettingsSection title="Recording">
        <SelectSetting label="Global shortcut">
          <Input
            id="hotkey-input"
            onChange={(event: ChangeEvent<HTMLInputElement>) =>
              update("hotkey", event.target.value)
            }
            type="text"
            value={settings.hotkey}
          />
        </SelectSetting>
        <SelectSetting label="Recording trigger">
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
        </SelectSetting>
        <SelectSetting label="Preferred mode">
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
        </SelectSetting>
      </SettingsSection>

      <SettingsSection title="Audio">
        <SelectSetting
          label="Microphone"
          description="Choose which input device Kiflo records from. Leave it on System default to follow macOS."
        >
          <select
            id="input-device-select"
            className={selectClass}
            onChange={(event: ChangeEvent<HTMLSelectElement>) =>
              update("inputDeviceId", event.target.value)
            }
            value={settings.inputDeviceId}
          >
            <option value="">System default</option>
            {inputDevices.map((device) => (
              <option key={device.id} value={device.id}>
                {device.isDefault ? `${device.name} (default)` : device.name}
              </option>
            ))}
          </select>
        </SelectSetting>
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
      </SettingsSection>

      <SettingsSection title="Text Processing">
        <SelectSetting
          label="Typing style"
          description="Go formal in docs, very casual in messages, or excited in upbeat notes."
        >
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
        </SelectSetting>
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
        <ToggleSetting
          id="restore-clipboard"
          checked={settings.restoreClipboard}
          onChange={(checked) => update("restoreClipboard", checked)}
          label="Restore clipboard after paste"
        />
        <ToggleSetting
          id="spoken-press-enter"
          checked={settings.spokenPressEnterCommand}
          onChange={(checked) => update("spokenPressEnterCommand", checked)}
          label='Enable spoken "press enter" at the end of a recording'
        />
      </SettingsSection>

      <SettingsSection title="History & Display">
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
      </SettingsSection>

      <SettingsSection title="Speech Model">
        <SelectSetting
          label="Speech model"
          description="Base English and Small English are bundled with the app. Small English is more accurate, but it uses more disk space and takes longer to warm up."
        >
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
        </SelectSetting>
      </SettingsSection>
    </div>
  );
}

function SettingsSection({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <div className="space-y-2">
      <h2 className="text-xs font-semibold uppercase tracking-wider text-muted-foreground px-1">
        {title}
      </h2>
      <div className="rounded-xl border bg-card divide-y divide-border overflow-hidden">
        {children}
      </div>
    </div>
  );
}

function SelectSetting({
  label,
  description,
  children,
}: {
  label: string;
  description?: string;
  children: React.ReactNode;
}) {
  return (
    <div className="flex flex-col gap-2 px-4 py-3">
      <Label>{label}</Label>
      {description && <p className="text-sm text-muted-foreground m-0">{description}</p>}
      {children}
    </div>
  );
}

interface ToggleSettingProps {
  id: string;
  checked: boolean;
  onChange: (checked: boolean) => void;
  label: string;
  description?: string;
}

function ToggleSetting({ id, checked, onChange, label, description }: ToggleSettingProps) {
  return (
    <div className="flex items-center justify-between gap-4 px-4 py-3">
      <div className="flex flex-col gap-0.5">
        <Label htmlFor={id}>{label}</Label>
        {description && <p className="text-sm text-muted-foreground m-0">{description}</p>}
      </div>
      <Switch id={id} checked={checked} onCheckedChange={onChange} />
    </div>
  );
}

function labelForSpeechModel(model: SpeechModel, bundledModels: SpeechModel[]): string {
  const baseLabel = model === "base_en" ? "Base English" : "Small English";
  return bundledModels.includes(model) ? `${baseLabel} (bundled)` : baseLabel;
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
