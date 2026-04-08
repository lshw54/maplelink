import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { App } from "./App";
import "./styles/globals.css";

const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      retry: 1,
      refetchOnWindowFocus: false,
    },
  },
});

// Disable browser-like behaviors to make it feel like a native app
document.addEventListener("contextmenu", (e) => {
  // Allow context menu only on our custom account cards (handled by React)
  if (!(e.target as HTMLElement).closest("[data-allow-context]")) {
    e.preventDefault();
  }
});

document.addEventListener("keydown", (e) => {
  // Disable F5 refresh, Ctrl+R refresh, Ctrl+Shift+I devtools (in production)
  if (e.key === "F5" || (e.ctrlKey && e.key === "r")) {
    e.preventDefault();
  }
});

const root = document.getElementById("root");
if (root) {
  createRoot(root).render(
    <StrictMode>
      <QueryClientProvider client={queryClient}>
        <App />
      </QueryClientProvider>
    </StrictMode>,
  );
}
