// SPDX-License-Identifier: MIT OR Apache-2.0
// Dark / light / match Windows. The choice is one of the user's preferences (prefs.tsx,
// stored in GMM's database); localStorage keeps a copy only so the first paint, before the
// database answers, already has the right theme. The theme class on <html> carries every
// token; dark is the default.

import { useEffect, useState } from "react";
import type { ThemeChoice } from "./api";

const KEY = "gmm.theme";

/** The copy from the last run, for the first paint. */
export function cachedTheme(): ThemeChoice {
  try {
    const v = localStorage.getItem(KEY);
    if (v === "dark" || v === "light" || v === "system") return v;
  } catch {
    // Storage can be unavailable; fall back to the default.
  }
  return "dark";
}

export function cacheTheme(choice: ThemeChoice) {
  try {
    localStorage.setItem(KEY, choice);
  } catch {
    // Not cached; the next start paints dark until the database answers.
  }
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

/** Apply the cached theme before the first render, so the window never flashes. */
export function applyCachedTheme() {
  apply(resolve(cachedTheme()));
}

/** Applies `choice` (following Windows while it is "system") and returns the theme in use. */
export function useAppliedTheme(choice: ThemeChoice): "dark" | "light" {
  const [theme, setTheme] = useState(() => resolve(choice));
  useEffect(() => {
    cacheTheme(choice);
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
  return theme;
}
