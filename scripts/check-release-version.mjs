// SPDX-License-Identifier: MIT OR Apache-2.0
// The release's versions must agree before an installer is built:
//   - the Rust workspace version (Cargo.toml), e.g. 0.0.1-beta: what GMM reports and
//     compares in its update check;
//   - the desktop frontend's (apps/desktop/package.json): the same;
//   - the installer's (apps/desktop/src-tauri/tauri.conf.json), e.g. 0.0.1: MSI versions
//     are numbers only, so it is the workspace version without its pre-release part;
//   - the git tag, when one is given: `v` + the workspace version.
// Usage: node scripts/check-release-version.mjs [tag]. Prints the workspace version.

import { readFileSync } from "node:fs";

const read = (path) => readFileSync(new URL(`../${path}`, import.meta.url), "utf8");

const cargo = read("Cargo.toml").match(/\[workspace\.package\][^[]*?\nversion = "([^"]+)"/)?.[1];
const pkg = JSON.parse(read("apps/desktop/package.json")).version;
const tauri = JSON.parse(read("apps/desktop/src-tauri/tauri.conf.json")).version;
const tag = process.argv[2];

const problems = [];
if (!cargo || !/^\d+\.\d+\.\d+(-[0-9A-Za-z.]+)?$/.test(cargo)) {
  problems.push(`Cargo.toml's workspace version isn't a semantic version: ${cargo}`);
}
if (pkg !== cargo) problems.push(`apps/desktop/package.json says ${pkg}, Cargo.toml says ${cargo}`);
const numeric = cargo?.split("-")[0];
if (tauri !== numeric) problems.push(`tauri.conf.json says ${tauri}; the installer needs ${numeric}`);
if (tag !== undefined && tag !== `v${cargo}`) problems.push(`the tag is ${tag}; this commit is v${cargo}`);

if (problems.length > 0) {
  for (const p of problems) console.error(p);
  process.exit(1);
}
console.log(cargo);
