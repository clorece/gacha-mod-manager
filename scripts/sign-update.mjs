// SPDX-License-Identifier: MIT OR Apache-2.0
// Signs a release's installer for GMM's updater (crates/gmm-core/src/app_update.rs): ed25519
// over "GMM update <version>\n<sha256 hex>\n", written as hex to `<installer>.sig`. The key is
// the repository secret GMM_UPDATE_KEY (PKCS#8 DER, base64). Refuses to sign when that key
// isn't the one whose public half GMM checks against (UPDATE_PUBLIC_KEY), since installed
// copies would then refuse the update.
// Usage: GMM_UPDATE_KEY=... node scripts/sign-update.mjs <installer.msi> <version>

import { createHash, createPrivateKey, createPublicKey, sign } from "node:crypto";
import { readFileSync, writeFileSync } from "node:fs";

const [installer, version] = process.argv.slice(2);
const secret = process.env.GMM_UPDATE_KEY?.trim();
if (!installer || !version) {
  console.error("usage: node scripts/sign-update.mjs <installer.msi> <version>");
  process.exit(2);
}
if (!secret) {
  console.error("GMM_UPDATE_KEY isn't set: updates must be signed");
  process.exit(1);
}

const key = createPrivateKey({ key: Buffer.from(secret, "base64"), format: "der", type: "pkcs8" });
const spki = createPublicKey(key).export({ type: "spki", format: "der" });
const publicHex = spki.subarray(spki.length - 32).toString("hex");
const source = readFileSync(new URL("../crates/gmm-core/src/app_update.rs", import.meta.url), "utf8");
const expected = source.match(/UPDATE_PUBLIC_KEY: &str =\s*"([0-9a-f]{64})"/)?.[1];
if (publicHex !== expected) {
  console.error(`GMM_UPDATE_KEY's public key is ${publicHex}; GMM checks against ${expected}`);
  process.exit(1);
}

const sha256 = createHash("sha256").update(readFileSync(installer)).digest("hex");
const signature = sign(null, Buffer.from(`GMM update ${version}\n${sha256}\n`), key).toString("hex");
writeFileSync(`${installer}.sig`, `${signature}\n`);
console.log(`signed ${installer} (${version}, sha256 ${sha256})`);
