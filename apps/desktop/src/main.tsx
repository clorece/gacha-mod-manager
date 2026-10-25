// SPDX-License-Identifier: MIT OR Apache-2.0
import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import App from "./App";
import { applyStoredTheme } from "./theme";
import { ToastProvider } from "./ui";
import "./styles.css";

applyStoredTheme();

const root = document.getElementById("root");
if (root) {
  createRoot(root).render(
    <StrictMode>
      <ToastProvider>
        <App />
      </ToastProvider>
    </StrictMode>,
  );
}
