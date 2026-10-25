// SPDX-License-Identifier: MIT OR Apache-2.0
// Dark / light / match Windows. The choice is a per-window convenience kept in localStorage
// (the theme class on <html> carries every token, docs/08); dark is the default.

import { useCallback, useEffect, useState } from "react";

export type ThemeChoice = "dark" | "light" | "system";
const KEY = "gmm.theme";

function stored(): ThemeChoice {
  try {
    const v = localStorage.getItem(KEY);
    if (v === "dark" || v === "light" || v === "system") return v;
  } catch {
    // Storage can be unavailable; fall back to the default.
  }
  return "dark";
}

const systemQuery = () => window.matchMedia("(prefers-color-scheme: light)");

function resolve(choice: ThemeChoice): "dark" | "light" {
  if (choice !== "system") return choice;
  return systemQuery().matches ? "light" : "dark";
}

function apply(theme: "dark" | "light") {
  const root = document.documentElement;
  root.classList.toggle("t-dark", theme === "dark");
  root.classList.toggle("t-light", theme === "light");
}

/** Apply the stored theme before the first render, so the window never flashes. */
export function applyStoredTheme() {
  apply(resolve(stored()));
}

export function useTheme() {
  const [choice, setChoice] = useState<ThemeChoice>(stored);
  const [theme, setTheme] = useState(() => resolve(choice));

  useEffect(() => {
    const update = () => {
      const t = resolve(choice);
      setTheme(t);
      apply(t);
    };
    update();
    if (choice !== "system") return;
    const q = systemQuery();
    q.addEventListener("change", update);
    return () => q.removeEventListener("change", update);
  }, [choice]);

  const choose = useCallback((next: ThemeChoice) => {
    setChoice(next);
    try {
      localStorage.setItem(KEY, next);
    } catch {
      // Not saved; the choice still applies to this window.
    }
  }, []);

  return { choice, theme, choose };
}
