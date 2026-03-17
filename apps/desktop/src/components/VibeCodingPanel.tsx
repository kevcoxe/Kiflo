import type { Settings } from "@kiflo/shared-types";
import { Label } from "@/components/ui/label";
import { Switch } from "@/components/ui/switch";

interface VibeCodingPanelProps {
  settings: Settings;
  onChange: (settings: Settings) => void;
}

export function VibeCodingPanel({ settings, onChange }: VibeCodingPanelProps) {
  function update<K extends keyof Settings>(key: K, value: Settings[K]) {
    onChange({ ...settings, [key]: value });
  }

  return (
    <div className="space-y-6 p-4">
      <div className="space-y-2">
        <h2 className="text-xs font-semibold uppercase tracking-wider text-muted-foreground px-1">
          Editor Integration
        </h2>
        <div className="rounded-xl border bg-card divide-y divide-border overflow-hidden">
          <ToggleSetting
            id="editor-file-mentions"
            checked={settings.editorFileMentionsEnabled}
            onChange={(checked) => update("editorFileMentionsEnabled", checked)}
            label="Enable editor file mentions"
            description="In supported editor chat boxes, convert spoken file names into @mentions and confirm them with Tab."
          />
          <ToggleSetting
            id="force-editor-composer"
            checked={settings.forceEditorComposer}
            onChange={(checked) => update("forceEditorComposer", checked)}
            label="Force supported editors to behave like composer fields"
            description="Use @mentions and Tab even when Kiflo cannot confirm the focused field is a chat/composer input."
          />
          <ToggleSetting
            id="show-editor-debug-hud"
            checked={settings.showEditorDebugHud}
            onChange={(checked) => update("showEditorDebugHud", checked)}
            label="Show editor detection debug info in history"
            description="Annotates each history entry with the app name, composer detection result, confidence score, and matched cues."
          />
        </div>
      </div>
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
