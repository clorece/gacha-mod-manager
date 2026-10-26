// SPDX-License-Identifier: MIT OR Apache-2.0
// Fails when an npm package in the desktop app's dependency tree has a license outside the
// policy in deny.toml (docs/licensing.md). Run from apps/desktop:
// `pnpm run check-licenses`.

import { execFileSync } from "node:child_process";

// Same list as deny.toml's [licenses].allow, plus 0BSD, BlueOak and MIT-0 (permissive, common in
// npm; MIT-0 is MIT without the attribution clause, reached through jsdom, a test-only dependency).
const ALLOWED = new Set([
  "MIT",
  "Apache-2.0",
  "BSD-2-Clause",
  "BSD-3-Clause",
  "ISC",
  "Zlib",
  "MPL-2.0",
  "Unicode-3.0",
  "CC0-1.0",
  "0BSD",
  "BlueOak-1.0.0",
  "MIT-0",
]);

// Per-package exceptions, as [name, license], each with a reason.
const EXCEPTIONS = new Map([
  // Font files only (no code): the UI font, Nunito. OFL-1.1 allows bundling it with software;
  // it can't be sold on its own. Listed in THIRD-PARTY-NOTICES.md (docs/licensing.md).
  ["@fontsource-variable/nunito", "OFL-1.1"],
]);

/** True when an SPDX expression is satisfied by allowed licenses (OR: any side; AND: all). */
function allowed(expr) {
  const e = expr.trim().replace(/^\((.*)\)$/, "$1");
  if (e.includes(" OR ")) return e.split(" OR ").some(allowed);
  if (e.includes(" AND ")) return e.split(" AND ").every(allowed);
  return ALLOWED.has(e);
}

// Run the same pnpm that runs this script (it may be a corepack shim, a JS file or a binary).
const pnpm = process.env.npm_execpath;
if (!pnpm) {
  console.error("run this through pnpm: `pnpm run check-licenses`");
  process.exit(2);
}
const [cmd, args] = /.[cm]?js$/.test(pnpm) ? [process.execPath, [pnpm]] : [pnpm, []];
const out = execFileSync(cmd, [...args, "licenses", "list", "--json"], {
  encoding: "utf8",
  maxBuffer: 64 * 1024 * 1024,
});
const byLicense = JSON.parse(out);
const problems = [];
let count = 0;
for (const [license, packages] of Object.entries(byLicense)) {
  for (const pkg of packages) {
    count += 1;
    if (allowed(license) || EXCEPTIONS.get(pkg.name) === license) continue;
    problems.push(`${pkg.name}@${pkg.versions.join(",")}: ${license}`);
  }
}
if (problems.length > 0) {
  console.error(`npm packages with licenses outside the policy:\n  ${problems.join("\n  ")}`);
  process.exit(1);
}
console.log(`${count} npm packages checked; all licenses are allowed.`);
