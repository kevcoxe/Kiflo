import { Sun, Moon, Monitor } from "lucide-react";
import { cn } from "@/lib/utils";
import type { SidebarSection } from "@/components/Sidebar";
import type { Settings } from "@kiflo/shared-types";

const SECTION_LABELS: Record<SidebarSection, string> = {
  record: "Record",
  dashboard: "Dashboard",
  settings: "Settings",
  "vibe-coding": "Vibe Coding",
  dictionary: "Dictionary",
  permissions: "Permissions",
};

interface TopBarProps {
  section: SidebarSection;
  theme: Settings["theme"];
  onThemeChange: (theme: Settings["theme"]) => void;
}

export function TopBar({ section, theme, onThemeChange }: TopBarProps) {
  return (
    <header className="flex items-center justify-between h-12 px-5 border-b border-border bg-card shrink-0">
      <span className="text-sm font-semibold">{SECTION_LABELS[section]}</span>
      <div className="flex items-center gap-1">
        <ThemeButton
          active={theme === "light"}
          icon={Sun}
          label="Light"
          onClick={() => onThemeChange("light")}
        />
        <ThemeButton
          active={theme === "system"}
          icon={Monitor}
          label="System"
          onClick={() => onThemeChange("system")}
        />
        <ThemeButton
          active={theme === "dark"}
          icon={Moon}
          label="Dark"
          onClick={() => onThemeChange("dark")}
        />
      </div>
    </header>
  );
}

interface ThemeButtonProps {
  active: boolean;
  icon: React.ComponentType<{ className?: string }>;
  label: string;
  onClick: () => void;
}

function ThemeButton({ active, icon: Icon, label, onClick }: ThemeButtonProps) {
  return (
    <button
      type="button"
      aria-label={`${label} theme`}
      aria-pressed={active}
      onClick={onClick}
      className={cn(
        "flex items-center justify-center w-8 h-8 rounded-md transition-colors",
        active
          ? "bg-primary text-primary-foreground"
          : "text-muted-foreground hover:bg-accent hover:text-foreground",
      )}
    >
      <Icon className="h-4 w-4" />
    </button>
  );
}
