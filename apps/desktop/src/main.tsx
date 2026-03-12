import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import App from "./App";
import { RecordingHud } from "@/components/RecordingHud";
import { currentDesktopWindowLabel } from "@/lib/desktop";
import "./app.css";

const rootElement = document.getElementById("root");
const windowLabel = currentDesktopWindowLabel();

if (!rootElement) {
  throw new Error("Root element #root was not found.");
}

document.documentElement.classList.toggle("hud-window", windowLabel === "hud");
document.body.classList.toggle("hud-window", windowLabel === "hud");

createRoot(rootElement).render(
  <StrictMode>
    {windowLabel === "hud" ? <RecordingHud /> : <App />}
  </StrictMode>,
);
