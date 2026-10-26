// SPDX-License-Identifier: MIT OR Apache-2.0
// Page state that outlives the page. Pages unmount when the user goes elsewhere; the Mods
// Store keeps its search, filters and loaded results here, so coming back finds it as it
// was (docs/08, "Mods Store"). Kept in memory only, for as long as the window is open.

import { type Dispatch, type SetStateAction, useCallback, useState } from "react";

const memory = new Map<string, unknown>();

/** `useState` whose value survives the component unmounting, under `key`. */
export function useRemembered<T>(key: string, initial: T | (() => T)): [T, Dispatch<SetStateAction<T>>] {
  const [value, setValue] = useState<T>(() => {
    if (memory.has(key)) return memory.get(key) as T;
    const first = typeof initial === "function" ? (initial as () => T)() : initial;
    memory.set(key, first);
    return first;
  });
  const set = useCallback<Dispatch<SetStateAction<T>>>(
    (next) =>
      setValue((current) => {
        const value = typeof next === "function" ? (next as (prev: T) => T)(current) : next;
        memory.set(key, value);
        return value;
      }),
    [key],
  );
  return [value, set];
}

/** Reads a remembered value without subscribing (`undefined` when there is none). */
export function recall<T>(key: string): T | undefined {
  return memory.get(key) as T | undefined;
}

/** Stores a value for the next `useRemembered` or `recall` of `key`. */
export function remember<T>(key: string, value: T): void {
  memory.set(key, value);
}

/** Forgets everything (tests start each case fresh). */
export function forgetAll(): void {
  memory.clear();
}
