// SPDX-License-Identifier: MIT OR Apache-2.0
import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import App from "./App";
import { api } from "./api";
import { PrefsProvider } from "./prefs";
import { applyCachedTheme } from "./theme";
import { ToastProvider } from "./ui";
import "./styles.css";

applyCachedTheme();

// Errors nobody handled go to GMM's local error log, for bug reports (Report a Bug).
const logUiError = (what: unknown) => {
  const text = what instanceof Error ? `${what.name}: ${what.message}` : String(what);
  api.logUiError(text).catch(() => {});
};
window.addEventListener("error", (e) => logUiError(e.error ?? e.message));
window.addEventListener("unhandledrejection", (e) => logUiError(e.reason));

const root = document.getElementById("root");
if (root) {
  createRoot(root).render(
    <StrictMode>
      <ToastProvider>
        <PrefsProvider>
          <App />
        </PrefsProvider>
      </ToastProvider>
    </StrictMode>,
  );
}
