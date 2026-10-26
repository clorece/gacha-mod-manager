// SPDX-License-Identifier: MIT OR Apache-2.0
// Renders a screen inside the providers the app wraps everything in (toasts, preferences).

import { render } from "@testing-library/react";
import type { ReactNode } from "react";
import { PrefsProvider } from "../prefs";
import { ToastProvider } from "../ui";

export function renderApp(ui: ReactNode) {
  return render(
    <ToastProvider>
      <PrefsProvider>{ui}</PrefsProvider>
    </ToastProvider>,
  );
}
