import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { DebugConsole } from "./features/debug/DebugConsole";
import "./styles/debug.css";

createRoot(document.getElementById("root") as HTMLElement).render(
  <StrictMode>
    <DebugConsole />
  </StrictMode>,
);
