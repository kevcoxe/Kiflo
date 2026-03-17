import { defaultWindowIcon } from "@tauri-apps/api/app";
import { Menu } from "@tauri-apps/api/menu";
import { TrayIcon, type TrayIconOptions } from "@tauri-apps/api/tray";

let trayIcon: TrayIcon | null = null;

interface TrayHandlers {
  onShowWindow: () => void;
  onToggleRecording: () => void;
  onQuit: () => void;
}

export async function ensureTray(handlers: TrayHandlers): Promise<void> {
  if (trayIcon) {
    return;
  }

  const menu = await Menu.new({
    items: [
      {
        id: "open",
        text: "Open Kiflo",
        action: () => handlers.onShowWindow(),
      },
      {
        id: "toggle",
        text: "Toggle Dictation",
        action: () => handlers.onToggleRecording(),
      },
      {
        id: "quit",
        text: "Quit",
        action: () => handlers.onQuit(),
      },
    ],
  });

  const options: TrayIconOptions = {
    menu,
    menuOnLeftClick: true,
    tooltip: "Kiflo",
    action: (event) => {
      if (event.type === "Click") {
        handlers.onShowWindow();
      }
    },
  };

  const icon = await defaultWindowIcon();
  if (icon) {
    options.icon = icon;
  }

  trayIcon = await TrayIcon.new(options);
}
