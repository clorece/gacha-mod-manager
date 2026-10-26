// SPDX-License-Identifier: MIT OR Apache-2.0
// Display formatting shared by every screen, and the library's mod counts.

import { describe, expect, it } from "vitest";
import { modCounts } from "./data";
import { ago, bytes, compact, plural } from "./format";
import { libraries, NOW } from "./test/fixtures";

describe("format", () => {
  it("formats sizes like the CLI", () => {
    expect(bytes(512)).toBe("512 B");
    expect(bytes(1536)).toBe("1.5 KiB");
    expect(bytes(131 * 1024 * 1024)).toBe("131.0 MiB");
  });

  it("counts compactly and pluralizes", () => {
    expect([950, 1200, 48_000, 3_100_000].map(compact)).toEqual(["950", "1.2k", "48k", "3.1M"]);
    expect(plural(1, "mod")).toBe("1 mod");
    expect(plural(3, "version")).toBe("3 versions");
  });

  it("says how long ago, then the date", () => {
    expect(ago(NOW - 30, NOW)).toBe("just now");
    expect(ago(NOW - 3 * 3600, NOW)).toBe("3 h ago");
    expect(ago(NOW - 86_400, NOW)).toBe("yesterday");
    expect(ago(NOW - 10 * 86_400, NOW)).toMatch(/2026/);
  });
});

describe("modCounts", () => {
  it("counts a version group as one mod, on when any version is", () => {
    // ZZZ: 3 single mods + 1 group of 3 versions + 2 hand-installed folders.
    expect(modCounts(libraries.zzz)).toEqual({ total: 6, on: 4 });
    expect(modCounts(undefined)).toEqual({ total: 0, on: 0 });
  });
});
