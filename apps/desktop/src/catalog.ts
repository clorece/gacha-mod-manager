// SPDX-License-Identifier: MIT OR Apache-2.0
// Shared by the Mods Store and the mod page: picking an image size, what the user already
// has installed of a GameBanana mod, and the labels for sorts and trending periods.

import { useEffect, useMemo, useState } from "react";
import { api, type Image, type ModSummary, type ModUpdateInfo, type Sort, type TopPeriod } from "./api";
import type { AppData } from "./data";

/** The smallest copy at least `width` pixels wide (the original when none is). */
export function imageAt(image: Image | undefined, width: number): string | undefined {
  if (!image) return undefined;
  return image.variants.find((v) => v.size.width >= width)?.url ?? image.url;
}

/** GameBanana flags it (visibility or content ratings). Mirrors `ModSummary::is_sensitive`. */
export function isSensitive(m: ModSummary): boolean {
  return m.visibility !== "show" || m.has_content_ratings;
}

export const SORTS: { value: Sort; label: string }[] = [
  { value: "default", label: "Recommended" },
  { value: "recently_updated", label: "Recently updated" },
  { value: "newest", label: "Newest" },
  { value: "most_downloaded", label: "Most downloaded" },
  { value: "most_liked", label: "Most liked" },
  { value: "most_viewed", label: "Most viewed" },
];

const PERIODS: Record<TopPeriod, string> = {
  today: "Trending today",
  week: "Trending this week",
  month: "Trending this month",
  three_months: "Top of 3 months",
  six_months: "Top of 6 months",
  year: "Top of the year",
  all_time: "All-time favourite",
};

/** The featured gallery's tag; a backfilled mod is "Popular now". */
export function periodLabel(period: TopPeriod | null): string {
  return period ? PERIODS[period] : "Popular now";
}

/** What the user has of one GameBanana mod. */
export type Owned = {
  game: string;
  /** Library row ids, one per installed version. */
  rows: number[];
  /** At least one version is switched on. */
  on: boolean;
  /** Installed as several versions (or one of several). */
  versions: boolean;
  update: boolean;
  conflict: boolean;
};

/** Installed GameBanana mods by mod id, from the libraries, the startup check's updates and
 *  the conflict reports (library row ids that clash with a mod that's on). */
export function ownedIndex(
  data: AppData,
  updates: ModUpdateInfo[],
  conflictRows: Record<string, Set<number>>,
): Map<number, Owned> {
  const out = new Map<number, Owned>();
  for (const game of data.games) {
    for (const row of data.libraries[game.id]?.managed ?? []) {
      if (row.gb_mod_id === null) continue;
      const owned = out.get(row.gb_mod_id) ?? {
        game: game.id,
        rows: [],
        on: false,
        versions: false,
        update: false,
        conflict: false,
      };
      owned.rows.push(row.id);
      owned.on ||= row.enabled;
      owned.versions ||= row.group !== null || owned.rows.length > 1;
      owned.conflict ||= conflictRows[game.id]?.has(row.id) ?? false;
      out.set(row.gb_mod_id, owned);
    }
  }
  for (const u of updates) {
    const owned = out.get(u.gb_mod_id);
    if (owned) owned.update = true;
  }
  return out;
}

/** [`ownedIndex`] kept current: conflict reports are re-read whenever a library changes. */
export function useOwned(data: AppData, updates: ModUpdateInfo[]): Map<number, Owned> {
  const [conflictRows, setConflictRows] = useState<Record<string, Set<number>>>({});
  // biome-ignore lint/correctness/useExhaustiveDependencies: a changed library (a switch, an install) can change the conflicts
  useEffect(() => {
    let live = true;
    Promise.all(
      data.games.map(async (g) => {
        const report = await api.conflicts(g.id).catch(() => null);
        const rows = new Set<number>();
        for (const c of report?.conflicts ?? []) {
          for (const ref of [c.a, c.b]) if (ref.type === "managed") rows.add(ref.id);
        }
        return [g.id, rows] as const;
      }),
    ).then((pairs) => live && setConflictRows(Object.fromEntries(pairs)));
    return () => {
      live = false;
    };
  }, [data.games, data.libraries]);
  return useMemo(() => ownedIndex(data, updates, conflictRows), [data, updates, conflictRows]);
}
