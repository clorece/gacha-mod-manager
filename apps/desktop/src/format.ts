// SPDX-License-Identifier: MIT OR Apache-2.0
// Display formatting shared by the screens.

/** Binary units, like the CLI: `131.0 MiB`. */
export function bytes(n: number): string {
  const units = ["B", "KiB", "MiB", "GiB", "TiB"];
  let value = n;
  let unit = 0;
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024;
    unit += 1;
  }
  return unit === 0 ? `${n} B` : `${value.toFixed(1)} ${units[unit]}`;
}

/** `24 Oct 2026`, from Unix seconds (install dates). */
export function day(unixSeconds: number): string {
  return new Date(unixSeconds * 1000).toLocaleDateString("en-GB", { day: "numeric", month: "short", year: "numeric" });
}

/** `1 mod`, `3 mods`. */
export function plural(n: number, one: string, many = `${one}s`): string {
  return `${n} ${n === 1 ? one : many}`;
}
