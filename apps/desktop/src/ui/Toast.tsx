// SPDX-License-Identifier: MIT OR Apache-2.0
// Toasts: short messages in the bottom-right corner that clear themselves. `useToast()` from
// anywhere under <ToastProvider>; the region is a polite live region for screen readers.

import { createContext, type ReactNode, useCallback, useContext, useMemo, useRef, useState } from "react";
import { IconButton } from "./buttons";

export type ToastTone = "info" | "ok" | "warn" | "bad";
type Toast = { id: number; tone: ToastTone; text: string };
type Push = (tone: ToastTone, text: string) => void;

const ToastContext = createContext<Push | null>(null);

/** How long a toast stays; errors stay longer so they can be read. */
const LIFETIME_MS = { info: 5000, ok: 5000, warn: 8000, bad: 10000 } as const;

export function ToastProvider({ children }: { children: ReactNode }) {
  const [toasts, setToasts] = useState<Toast[]>([]);
  const next = useRef(1);

  const dismiss = useCallback((id: number) => setToasts((all) => all.filter((t) => t.id !== id)), []);

  const push = useCallback<Push>(
    (tone, text) => {
      const id = next.current++;
      setToasts((all) => [...all.slice(-3), { id, tone, text }]);
      setTimeout(() => dismiss(id), LIFETIME_MS[tone]);
    },
    [dismiss],
  );

  const value = useMemo(() => push, [push]);
  return (
    <ToastContext.Provider value={value}>
      {children}
      <section className="toasts" aria-live="polite" aria-label="Notifications">
        {toasts.map((t) => (
          <div key={t.id} className={`toast toast-${t.tone}`} role={t.tone === "bad" ? "alert" : "status"}>
            <span className="grow">{t.text}</span>
            <IconButton icon="close" label="Dismiss" size="sm" onClick={() => dismiss(t.id)} />
          </div>
        ))}
      </section>
    </ToastContext.Provider>
  );
}

export function useToast(): Push {
  const push = useContext(ToastContext);
  if (!push) throw new Error("useToast needs a <ToastProvider> above it");
  return push;
}
