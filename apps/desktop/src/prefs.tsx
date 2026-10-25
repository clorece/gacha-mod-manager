// SPDX-License-Identifier: MIT OR Apache-2.0
// The user's preferences (theme, sensitive mods, first run), from GMM's database. One copy
// for the whole app: the first-run screen, Settings and the sensitive-mods switch in the
// Store and Library filters all change the same values, so they always agree.

import { createContext, type ReactNode, useCallback, useContext, useEffect, useMemo, useRef, useState } from "react";
import { api, asCmdError, type Preferences } from "./api";
import { cachedTheme } from "./theme";
import { useToast } from "./ui";

/** Until the database answers (or if it can't): safe defaults, with last run's theme. */
const fallback = (): Preferences => ({
  theme: cachedTheme(),
  show_sensitive: false,
  blur_sensitive: true,
  onboarded: true,
});

type Prefs = {
  prefs: Preferences;
  /** False until the saved preferences arrived. */
  loaded: boolean;
  update: (change: Partial<Preferences>) => Promise<void>;
};

const PrefsContext = createContext<Prefs | null>(null);

export function PrefsProvider({ children }: { children: ReactNode }) {
  const toast = useToast();
  const [prefs, setPrefs] = useState<Preferences>(fallback);
  const [loaded, setLoaded] = useState(false);
  // The latest values, so quick successive changes build on each other.
  const current = useRef(prefs);
  current.current = prefs;

  useEffect(() => {
    api
      .preferences()
      .then(setPrefs)
      .catch(() => {})
      .finally(() => setLoaded(true));
  }, []);

  const update = useCallback(
    async (change: Partial<Preferences>) => {
      const next = { ...current.current, ...change };
      current.current = next;
      setPrefs(next);
      try {
        await api.setPreferences(next);
      } catch (e) {
        toast("bad", `Couldn't save that setting: ${asCmdError(e).message}`);
      }
    },
    [toast],
  );

  const value = useMemo(() => ({ prefs, loaded, update }), [prefs, loaded, update]);
  return <PrefsContext.Provider value={value}>{children}</PrefsContext.Provider>;
}

export function usePrefs(): Prefs {
  const value = useContext(PrefsContext);
  if (!value) throw new Error("usePrefs outside PrefsProvider");
  return value;
}
