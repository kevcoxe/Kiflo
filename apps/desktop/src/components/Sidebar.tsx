import { Mic, LayoutDashboard, History, Settings, Shield } from "lucide-react";
import { cn } from "@/lib/utils";

export type SidebarSection = "record" | "dashboard" | "history" | "settings" | "permissions";

const NAV_ITEMS: { id: SidebarSection; label: string; icon: React.ComponentType<{ className?: string }> }[] = [
  { id: "record", label: "Record", icon: Mic },
  { id: "dashboard", label: "Dashboard", icon: LayoutDashboard },
  { id: "history", label: "History", icon: History },
  { id: "settings", label: "Settings", icon: Settings },
  { id: "permissions", label: "Permissions", icon: Shield },
];

interface SidebarProps {
  active: SidebarSection;
  onSelect: (section: SidebarSection) => void;
}

export function Sidebar({ active, onSelect }: SidebarProps) {
  return (
    <aside className="flex flex-col w-52 shrink-0 border-r border-border bg-card min-h-screen">
      <div className="px-4 py-5 border-b border-border">
        <span className="text-base font-bold tracking-tight">Tator</span>
      </div>
      <nav className="flex flex-col gap-1 p-2 flex-1">
        {NAV_ITEMS.map(({ id, label, icon: Icon }) => (
          <button
            key={id}
            type="button"
            onClick={() => onSelect(id)}
            className={cn(
              "flex items-center gap-3 rounded-md px-3 py-2 text-sm font-medium transition-colors text-left w-full",
              active === id
                ? "bg-primary text-primary-foreground"
                : "text-muted-foreground hover:bg-accent hover:text-foreground",
            )}
          >
            <Icon className="h-4 w-4 shrink-0" />
            {label}
          </button>
        ))}
      </nav>
    </aside>
  );
}
