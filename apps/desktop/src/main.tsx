// SPDX-License-Identifier: MIT OR Apache-2.0
import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import App from "./App";
import { PrefsProvider } from "./prefs";
import { applyCachedTheme } from "./theme";
import { ToastProvider } from "./ui";
import "./styles.css";

applyCachedTheme();

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
