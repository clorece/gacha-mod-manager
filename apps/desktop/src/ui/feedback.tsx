// SPDX-License-Identifier: MIT OR Apache-2.0
// Small display pieces: badges, chips, notices, the progress bar and the spinner.

import type { ReactNode } from "react";
import { Icon, type IconName } from "./Icon";

/** `art` and `game` sit on artwork; `quiet` is the WIP tag; the rest carry a state. */
export type BadgeTone = "art" | "ok" | "game" | "yellow" | "quiet" | "warn" | "bad";

export function Badge({ tone, title, children }: { tone: BadgeTone; title?: string; children: ReactNode }) {
  return (
    <span className={`badge badge-${tone}`} title={title}>
      {children}
    </span>
  );
}

export type Tone = "ok" | "warn" | "bad";

/** A pill with an optional state dot or icon (summary chips in page headers). */
export function Chip({ dot, icon, children }: { dot?: Tone; icon?: IconName; children: ReactNode }) {
  return (
    <span className="chip">
      {dot && <span className={`dot${dot === "ok" ? "" : ` dot-${dot}`}`} aria-hidden="true" />}
      {icon && <Icon name={icon} size={15} stroke={2.4} />}
      {children}
    </span>
  );
}

/** An inline banner: a conflict, a failed action, a setup warning. */
export function Notice({
  tone,
  title,
  children,
  actions,
}: {
  tone: Tone | "info";
  title: ReactNode;
  children?: ReactNode;
  actions?: ReactNode;
}) {
  const icon: IconName = tone === "ok" ? "check" : tone === "info" ? "info" : "warning";
  return (
    <div className={`notice notice-${tone}`} role={tone === "bad" || tone === "warn" ? "alert" : "status"}>
      <Icon name={icon} size={22} stroke={2.2} />
      <div className="grow">
        <b>{title}</b> {children && <span className="sub">{children}</span>}
      </div>
      {actions}
    </div>
  );
}

/** Determinate when `value` (0 to 1) is given, otherwise the sliding indeterminate bar. */
export function ProgressBar({ value, label }: { value?: number; label: string }) {
  const pct = value === undefined ? undefined : Math.round(Math.max(0, Math.min(1, value)) * 100);
  return (
    <div
      className={`bar${pct === undefined ? " bar-ind" : ""}`}
      role="progressbar"
      aria-label={label}
      aria-valuemin={0}
      aria-valuemax={100}
      aria-valuenow={pct}
    >
      <span style={pct === undefined ? undefined : { width: `${pct}%` }} />
    </div>
  );
}

export function Spinner() {
  return <span className="spin" aria-hidden="true" />;
}
