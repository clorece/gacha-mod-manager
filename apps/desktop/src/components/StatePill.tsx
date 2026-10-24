// SPDX-License-Identifier: MIT OR Apache-2.0
import type { SetupState } from "../api";

/** Green "Ready", amber "Needs setup", red "XXMI not found" (docs/07, app shell). */
export function StatePill({ state }: { state: SetupState }) {
  const [label, tone] =
    state === "ready"
      ? ["Ready", "border-ok/50 bg-ok/10 text-ok"]
      : state === "xxmi_not_found"
        ? ["XXMI not found", "border-bad/50 bg-bad/10 text-bad"]
        : ["Needs setup", "border-warn/50 bg-warn/10 text-warn"];
  return <span className={`rounded-full border px-2.5 py-0.5 font-medium text-xs ${tone}`}>{label}</span>;
}
