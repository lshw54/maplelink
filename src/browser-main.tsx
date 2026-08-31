import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { BrowserChrome } from "./features/browser/BrowserChrome";
import "./styles/globals.css";

createRoot(document.getElementById("root") as HTMLElement).render(
  <StrictMode>
    <BrowserChrome />
  </StrictMode>,
);
