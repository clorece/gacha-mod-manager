// SPDX-License-Identifier: MIT OR Apache-2.0
// Mods Store (docs/08, "Mods Store"; docs/07): the featured gallery, a sticky toolbar
// (search, Category, Characters, Sort, Hide installed, Show sensitive) and a virtualized grid
// of GameBanana mods that pages on as it scrolls. Download counts aren't in listings, so
// they fill in for the cards on screen (`catalog_stats`). A click on a card opens GMM's mod
// page; its round button opens the install confirmation (or, for a mod already installed,
// its settings or "Add versions"). With sensitive mods hidden a page can come back nearly
// empty, so the grid keeps asking for more until the screen is full. Search, filters, results
// and the gallery are remembered (remember.ts) while the user is on other pages.

import { useVirtualizer } from "@tanstack/react-virtual";
import { memo, useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import {
  api,
  asCmdError,
  type Category,
  type FeaturedMod,
  type GameInfo,
  type ModStats,
  type ModSummary,
  type Sort,
} from "../api";
import { isSensitive, type Owned, periodLabel, SORTS } from "../catalog";
import { Preview } from "../components/Preview";
import type { AppData } from "../data";
import { compact } from "../format";
import { ALL, type Scope } from "../games";
import { usePrefs } from "../prefs";
import { useRemembered } from "../remember";
import { Badge, Button, Dropdown, Icon, IconButton, Notice, ToggleChip, useToast } from "../ui";

/** Cards asked for per page (split between characters when several are ticked). */
const PER_PAGE = 30;
/** Characters ticked at once (`gmm_core::MAX_BROWSE_STREAMS`). */
const MAX_CHARACTERS = 6;
/** Pages read in a row to fill the screen when sensitive or installed mods are hidden. */
const MAX_FILL_PAGES = 12;
const CARD_MIN = 232;
const GAP = 18;
/** Card body under the 16:10 preview: title, author, stats row, padding. */
const CARD_BODY = 146;
/** Most download counts asked for at once (`catalog_stats` takes up to 60). */
const STATS_BATCH = 60;

type Results = {
  items: ModSummary[];
  page: number;
  isLast: boolean;
  total: number;
  hidden: number;
  loading: boolean;
  error: string | null;
};

const fresh: Results = { items: [], page: 0, isLast: false, total: 0, hidden: 0, loading: true, error: null };

function useDebounced<T>(value: T, ms: number): T {
  const [out, setOut] = useState(value);
  useEffect(() => {
    const t = setTimeout(() => setOut(value), ms);
    return () => clearTimeout(t);
  }, [value, ms]);
  return out;
}

export function Store({
  data,
  scope,
  owned,
  onOpenMod,
  onDownload,
}: {
  data: AppData;
  scope: Scope;
  owned: Map<number, Owned>;
  onOpenMod: (modId: number) => void;
  onDownload: (mod: { id: number; name: string }) => void;
}) {
  const { prefs, update } = usePrefs();
  const toast = useToast();
  const game = scope === ALL ? null : (data.games.find((g) => g.id === scope) ?? null);

  // Search, filters and results are remembered while the user is on other pages.
  const [text, setText] = useRemembered("store.text", "");
  const query = useDebounced(text.trim(), 350);
  const [category, setCategoryRaw] = useRemembered<number | null>("store.category", null);
  const [characters, setCharacters] = useRemembered<number[]>("store.characters", []);
  const [sort, setSort] = useRemembered<Sort>("store.sort", "default");
  const [hideInstalled, setHideInstalled] = useRemembered("store.hideInstalled", false);
  const show = prefs.show_sensitive;
  /** Characters belong to a category. */
  const setCategory = (next: number | null) => {
    setCategoryRaw(next);
    setCharacters([]);
  };

  // A category belongs to one game: a new game scope starts without one.
  const [filtersScope, setFiltersScope] = useRemembered<Scope>("store.scope", scope);
  if (filtersScope !== scope) {
    setFiltersScope(scope);
    setCategoryRaw(null);
    setCharacters([]);
  }

  const cat = game?.categories.find((c) => c.id === category) ?? null;
  const [charOptions, setCharOptions] = useRemembered<Category[]>("store.charOptions", []);
  const catId = cat?.characters ? cat.id : null;
  // biome-ignore lint/correctness/useExhaustiveDependencies: the options follow the game and the category's id, not object identity
  useEffect(() => {
    if (!game || catId === null) {
      setCharOptions([]);
      return;
    }
    let live = true;
    api
      .catalogCategories(game.id, catId)
      .then((list) => live && setCharOptions(list.filter((c) => !c.obsolete)))
      .catch(() => live && setCharOptions([]));
    return () => {
      live = false;
    };
  }, [game?.id, catId]);

  // ---- results ---------------------------------------------------------------------------
  const [results, setResults] = useRemembered<Results>("store.results", fresh);
  /** The filters the remembered results were loaded for. */
  const [loadedFor, setLoadedFor] = useRemembered("store.loadedFor", "");
  const generation = useRef(0);
  // The latest results and whether a page is on its way, for the scroll and fill triggers
  // (two can fire before React re-renders).
  const latest = useRef(results);
  latest.current = results;
  const inflight = useRef(false);
  const filters = useMemo(
    () => ({
      game: scope,
      sort,
      categories: characters.length > 0 ? characters : category !== null ? [category] : [],
      text: query === "" ? null : query,
      show_nsfw: show,
    }),
    [scope, sort, characters, category, query, show],
  );

  const loadPage = useCallback(
    async (page: number) => {
      const gen = generation.current;
      inflight.current = true;
      setResults((r) => ({ ...r, loading: true, error: null }));
      try {
        const res = await api.catalogList({ ...filters, page, per_page: PER_PAGE });
        if (gen !== generation.current) return;
        setResults((r) => {
          const seen = new Set(r.items.map((m) => m.id));
          const added = res.page.items.filter((m) => !seen.has(m.id));
          return {
            items: page === 1 ? res.page.items : [...r.items, ...added],
            page,
            isLast: res.page.is_last || res.page.items.length + res.hidden === 0,
            total: res.page.total,
            hidden: (page === 1 ? 0 : r.hidden) + res.hidden,
            loading: false,
            error: null,
          };
        });
      } catch (e) {
        if (gen !== generation.current) return;
        setResults((r) => ({ ...r, loading: false, error: asCmdError(e).message }));
      } finally {
        if (gen === generation.current) inflight.current = false;
      }
    },
    [filters, setResults],
  );

  const filtersKey = JSON.stringify(filters);
  // biome-ignore lint/correctness/useExhaustiveDependencies: reload when the filters change, not when the remembered key does
  useEffect(() => {
    // Back on the Store with the same filters: keep what was loaded (and where it was).
    const kept = latest.current;
    if (loadedFor === filtersKey && kept.page > 0 && !kept.loading && !kept.error) return;
    generation.current += 1;
    inflight.current = false;
    setLoadedFor(filtersKey);
    setResults(fresh);
    void loadPage(1);
  }, [loadPage]);

  const loadMore = useCallback(() => {
    const r = latest.current;
    if (inflight.current || r.isLast || r.error || r.page === 0) return;
    void loadPage(r.page + 1);
  }, [loadPage]);

  const visible = useMemo(
    () => (hideInstalled ? results.items.filter((m) => !owned.has(m.id)) : results.items),
    [results.items, hideInstalled, owned],
  );
  const installedHidden = results.items.length - visible.length;

  // Fill the first screen when filters dropped most of a page.
  useEffect(() => {
    if (!results.loading && !results.isLast && !results.error && results.page > 0) {
      if (visible.length < PER_PAGE / 2 && results.page < MAX_FILL_PAGES) loadMore();
    }
  }, [results, visible.length, loadMore]);

  // ---- download counts -------------------------------------------------------------------
  const [stats, setStats] = useRemembered<Record<number, ModStats>>("store.stats", {});
  const requested = useRef(new Set<number>(Object.keys(stats).map(Number)));
  const pendingIds = useRef<number[]>([]);
  const statsTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const arrived = useRef<Record<number, ModStats>>({});
  const flush = useRef<number | null>(null);
  const wantStats = useCallback(
    (ids: number[]) => {
      pendingIds.current = ids.filter((id) => !requested.current.has(id));
      if (statsTimer.current) clearTimeout(statsTimer.current);
      statsTimer.current = setTimeout(() => {
        const batch = pendingIds.current.slice(0, STATS_BATCH);
        if (batch.length === 0) return;
        for (const id of batch) requested.current.add(id);
        api
          .catalogStats(batch, (s) => {
            // Counts arrive one mod at a time; draw them once per frame, not once each.
            arrived.current[s.mod_id] = s.stats;
            if (flush.current === null) {
              flush.current = requestAnimationFrame(() => {
                flush.current = null;
                const add = arrived.current;
                arrived.current = {};
                setStats((all) => ({ ...all, ...add }));
              });
            }
          })
          .catch(() => {
            // Cards keep their placeholder; scrolling back asks again.
            for (const id of batch) requested.current.delete(id);
          });
      }, 300);
    },
    [setStats],
  );

  // ---- featured gallery ------------------------------------------------------------------
  const [featured, setFeatured] = useRemembered<FeaturedMod[]>("store.featured", []);
  const [featuredFor, setFeaturedFor] = useRemembered("store.featuredFor", "");
  // biome-ignore lint/correctness/useExhaustiveDependencies: the gallery follows the scope, the game list and the sensitive switch
  useEffect(() => {
    const games = game ? [game] : data.games;
    const key = `${games.map((g) => g.id).join(",")}:${show}`;
    if (key === featuredFor && featured.length > 0) return;
    let live = true;
    setFeaturedFor(key);
    setFeatured([]);
    Promise.all(games.map((g) => api.catalogFeatured(g.id, show).catch(() => [] as FeaturedMod[]))).then((lists) => {
      if (!live) return;
      // Interleave games under "All games".
      const out: FeaturedMod[] = [];
      for (let i = 0; lists.some((l) => i < l.length); i++) {
        for (const l of lists) if (l[i]) out.push(l[i]);
      }
      setFeatured(out);
    });
    return () => {
      live = false;
    };
  }, [game?.id, data.games, show]);

  const pickCharacters = (next: number[]) => {
    if (next.length > MAX_CHARACTERS) {
      toast("info", `Up to ${MAX_CHARACTERS} characters at once.`);
      return;
    }
    setCharacters(next);
  };

  const resetFilters = () => {
    setText("");
    setCategory(null);
    setCharacters([]);
    setSort("default");
    setHideInstalled(false);
  };

  const charKind = cat?.name.toLowerCase().includes("bangboo") ? "Bangboo" : "Characters";
  const charReason = !game
    ? "Pick a game to filter by character"
    : !cat
      ? "Pick a skins category first"
      : !cat.characters
        ? `${cat.name} isn't sorted by character`
        : undefined;
  const filtered = query !== "" || category !== null || hideInstalled;
  const gameName = (id: number | null) => data.games.find((g) => g.gamebanana_id === id) ?? null;

  return (
    <>
      <div className="page-head">
        <div>
          <h1>Mods Store</h1>
          <p>
            {game ? `${game.name} mods` : "Mods for every game"} from GameBanana. Nothing installs until you confirm.
          </p>
        </div>
      </div>

      {featured.length > 0 && query === "" && (
        <Featured
          picks={featured}
          showGame={!game}
          gameOf={gameName}
          owned={owned}
          onOpen={onOpenMod}
          onDownload={onDownload}
        />
      )}

      <div className="filterbar">
        <div className="toolbar">
          <div className="search">
            <label className="sr-only" htmlFor="store-q">
              Search mods
            </label>
            <Icon name="search" size={18} stroke={2.4} />
            <input
              id="store-q"
              type="search"
              placeholder={`Search ${game ? game.name : "all games"}`}
              value={text}
              onChange={(e) => setText(e.target.value)}
            />
          </div>
          <Dropdown<number>
            label="Category"
            value={category ?? 0}
            onChange={(v) => setCategory(v === 0 ? null : v)}
            options={[
              { value: 0, label: "All mods" },
              ...(game?.categories ?? []).map((c) => ({ value: c.id, label: c.name })),
            ]}
            disabled={!game}
            disabledReason="Pick a game to filter by category"
          />
          <Dropdown<number>
            multiple
            label={charKind}
            allLabel={`All ${charKind.toLowerCase()}`}
            value={characters}
            onChange={pickCharacters}
            options={charOptions.map((c) => ({
              value: c.id,
              label: c.name,
              icon: c.icon_url ? (
                <img className="ch-icon" src={c.icon_url} alt="" />
              ) : (
                <span className="ch-icon ch-av" aria-hidden="true">
                  {c.name.slice(0, 2)}
                </span>
              ),
            }))}
            disabled={charReason !== undefined || charOptions.length === 0}
            disabledReason={charReason ?? "Loading…"}
          />
          <Dropdown<Sort> label="Sort" value={sort} onChange={setSort} options={SORTS} />
          <ToggleChip label="Hide installed" checked={hideInstalled} onChange={setHideInstalled} />
          <ToggleChip
            label="Show sensitive"
            checked={show}
            onChange={(next) => void update({ show_sensitive: next })}
          />
          <span className="spacer" />
          <span className="count">
            {results.page > 0 ? `${results.total.toLocaleString("en-GB")} mods` : "Loading…"}
            {results.hidden > 0 && ` · ${results.hidden} hidden as sensitive`}
            {installedHidden > 0 && ` · ${installedHidden} installed hidden`}
          </span>
        </div>
      </div>

      {results.error && (
        <Notice
          tone="bad"
          title="Couldn't load mods from GameBanana."
          actions={<Button onClick={() => void loadPage(Math.max(results.page, 0) + 1)}>Try again</Button>}
        >
          {results.error}
        </Notice>
      )}

      {!results.loading && !results.error && visible.length === 0 ? (
        <div className="card empty">
          {filtered || characters.length > 0 ? "No mods match these filters." : "No mods to show."}
          {(filtered || characters.length > 0 || sort !== "default") && (
            <div className="mt-3">
              <Button onClick={resetFilters}>Reset filters</Button>
            </div>
          )}
        </div>
      ) : (
        <StoreGrid
          items={visible}
          loading={results.loading}
          isLast={results.isLast}
          owned={owned}
          stats={stats}
          showGame={!game}
          gameOf={gameName}
          onNearEnd={loadMore}
          onVisible={wantStats}
          onOpen={onOpenMod}
          onDownload={onDownload}
        />
      )}
      {results.isLast && visible.length > 0 && <p className="end-note">That's every mod for these filters.</p>}
    </>
  );
}

// ---- featured gallery ----------------------------------------------------------------------

function Featured({
  picks,
  showGame,
  gameOf,
  owned,
  onOpen,
  onDownload,
}: {
  picks: FeaturedMod[];
  showGame: boolean;
  gameOf: (gbGameId: number | null) => GameInfo | null;
  owned: Map<number, Owned>;
  onOpen: (modId: number) => void;
  onDownload: (mod: { id: number; name: string }) => void;
}) {
  const [index, setIndex] = useRemembered("store.featuredIndex", 0);
  const at = Math.min(index, picks.length - 1);
  const pick = picks[at];
  const m = pick.summary;
  const step = (d: number) => setIndex((at + d + picks.length) % picks.length);
  const strip = useRef<HTMLDivElement>(null);
  const [shift, setShift] = useState(0);

  // Slide the strip so the current banner leads, without running past its end.
  useLayoutEffect(() => {
    const el = strip.current;
    const item = el?.children[at] as HTMLElement | undefined;
    if (!el || !item || !el.parentElement) return;
    const max = Math.max(0, el.scrollWidth - el.parentElement.clientWidth);
    setShift(Math.min(item.offsetLeft, max));
  }, [at]);

  const g = showGame ? gameOf(m.game_id) : null;
  const mine = owned.get(m.id);
  return (
    <section className="featured" aria-label="Featured mods">
      <div className="hero">
        <Preview image={m.images[0]} width={1100} sensitive={isSensitive(m)} alt={m.name} className="hero-art" eager />
        <div className="hero-fade" />
        <div className="hero-body">
          <span className="hero-tag">
            <Icon name="trending" size={14} stroke={2.8} />
            {periodLabel(pick.period)}
            {g && ` · ${g.short_name}`}
          </span>
          <button type="button" className="hero-title" onClick={() => onOpen(m.id)}>
            {m.name}
          </button>
          <div className="hero-by">
            by {m.submitter.name}
            {m.category && ` · ${m.category.name}`}
          </div>
          <div className="hero-stats">
            <span title="Likes">
              <Icon name="heart" size={16} stroke={2.4} />
              {compact(m.stats.likes)}
            </span>
            <span title="Views">
              <Icon name="eye" size={16} stroke={2.4} />
              {compact(m.stats.views)}
            </span>
            {m.stats.downloads !== null && (
              <span title="Downloads">
                <Icon name="download" size={16} stroke={2.4} />
                {compact(m.stats.downloads)}
              </span>
            )}
          </div>
          <div className="flex gap-2.5">
            <Button
              variant="primary"
              icon={mine ? "check" : "download"}
              onClick={() => onDownload({ id: m.id, name: m.name })}
            >
              {mine ? (mine.versions ? "Add versions" : "Installed") : "Download"}
            </Button>
            <Button onClick={() => onOpen(m.id)}>View details</Button>
          </div>
        </div>
        {picks.length > 1 && (
          <div className="hero-nav">
            <IconButton icon="chevronLeft" label="Previous featured mod" variant="art" onClick={() => step(-1)} />
            <IconButton icon="chevronRight" label="Next featured mod" variant="art" onClick={() => step(1)} />
          </div>
        )}
      </div>
      {picks.length > 1 && (
        <div className="strip-wrap">
          <div className="strip" ref={strip} style={{ transform: `translateX(-${shift}px)` }}>
            {picks.map((p, i) => (
              <button
                key={p.summary.id}
                type="button"
                className={`strip-item${i === at ? " is-current" : ""}`}
                aria-current={i === at ? "true" : undefined}
                aria-label={`Feature ${p.summary.name}`}
                onClick={() => setIndex(i)}
              >
                <Preview
                  image={p.summary.images[0]}
                  width={200}
                  sensitive={isSensitive(p.summary)}
                  alt=""
                  className="strip-art"
                />
                <span className="fade" />
                <span className="strip-name">{p.summary.name}</span>
              </button>
            ))}
          </div>
        </div>
      )}
    </section>
  );
}

// ---- the grid ------------------------------------------------------------------------------

function StoreGrid({
  items,
  loading,
  isLast,
  owned,
  stats,
  showGame,
  gameOf,
  onNearEnd,
  onVisible,
  onOpen,
  onDownload,
}: {
  items: ModSummary[];
  loading: boolean;
  isLast: boolean;
  owned: Map<number, Owned>;
  stats: Record<number, ModStats>;
  showGame: boolean;
  gameOf: (gbGameId: number | null) => GameInfo | null;
  onNearEnd: () => void;
  onVisible: (ids: number[]) => void;
  onOpen: (modId: number) => void;
  onDownload: (mod: { id: number; name: string }) => void;
}) {
  const wrap = useRef<HTMLDivElement>(null);
  const scrollEl = useRef<HTMLElement | null>(null);
  // The last layout, so the grid comes back at its size when the Store is opened again.
  const [width, setWidth] = useRemembered("store.gridWidth", 0);
  const [margin, setMargin] = useRemembered("store.gridMargin", 0);

  useLayoutEffect(() => {
    scrollEl.current = wrap.current?.closest("main") ?? null;
    const el = wrap.current;
    if (!el) return;
    // While a mod page covers the Store the grid is hidden (width 0): keep the last layout,
    // so Back finds the same rows at the same height.
    const measure = () => {
      if (el.clientWidth > 0) setWidth(el.clientWidth);
    };
    // Measured on the next frame: setting the width inside the observer's callback changes the
    // layout it just reported, which the browser flags as a "ResizeObserver loop".
    let frame = 0;
    const ro = new ResizeObserver(() => {
      cancelAnimationFrame(frame);
      frame = requestAnimationFrame(measure);
    });
    ro.observe(el);
    measure();
    return () => {
      cancelAnimationFrame(frame);
      ro.disconnect();
    };
  }, [setWidth]);

  // Where the grid starts inside the scrolling page (the gallery above can change it).
  useLayoutEffect(() => {
    const el = wrap.current;
    const main = scrollEl.current;
    if (!el || !main || el.offsetParent === null) return;
    const top = el.getBoundingClientRect().top - main.getBoundingClientRect().top + main.scrollTop;
    if (Math.abs(top - margin) > 1) setMargin(top);
  });

  const cols = Math.max(1, Math.floor((width + GAP) / (CARD_MIN + GAP)));
  const colWidth = width > 0 ? (width - GAP * (cols - 1)) / cols : CARD_MIN;
  const rowHeight = Math.round((colWidth * 10) / 16) + CARD_BODY + GAP;
  const dataRows = Math.ceil(items.length / cols);
  const skeletonRows = loading && !isLast ? (items.length === 0 ? 2 : 1) : 0;
  const count = dataRows + skeletonRows;

  const virtualizer = useVirtualizer({
    count,
    getScrollElement: () => scrollEl.current,
    estimateSize: () => rowHeight,
    // A hidden row (a mod page covers the Store) measures 0; keep the estimate instead, or
    // the virtualizer "corrects" the scroll position when the Store shows again.
    measureElement: (el) => (el.getClientRects().length === 0 ? rowHeight : el.getBoundingClientRect().height),
    overscan: 2,
    scrollMargin: margin,
  });
  // biome-ignore lint/correctness/useExhaustiveDependencies: re-measure when the column layout changes
  useEffect(() => virtualizer.measure(), [rowHeight, cols]);

  // Stable for the memoized cards, whatever the page passes.
  const open = useStable(onOpen);
  const download = useStable(onDownload);

  const rows = virtualizer.getVirtualItems();
  const first = rows[0]?.index ?? 0;
  const last = rows[rows.length - 1]?.index ?? -1;

  useEffect(() => {
    if (last >= dataRows - 2) onNearEnd();
  }, [last, dataRows, onNearEnd]);

  useEffect(() => {
    const ids = items.slice(first * cols, (last + 1) * cols).map((m) => m.id);
    if (ids.length > 0) onVisible(ids);
  }, [first, last, cols, items, onVisible]);

  return (
    <div ref={wrap} className="vgrid" style={{ height: virtualizer.getTotalSize() }}>
      {rows.map((row) => {
        const start = row.index * cols;
        const cells = row.index < dataRows ? items.slice(start, start + cols) : null;
        return (
          <div
            key={row.key}
            ref={virtualizer.measureElement}
            data-index={row.index}
            className="vgrid-row"
            style={{
              transform: `translateY(${row.start - virtualizer.options.scrollMargin}px)`,
              gridTemplateColumns: `repeat(${cols}, minmax(0, 1fr))`,
            }}
          >
            {cells
              ? cells.map((m) => (
                  <StoreCard
                    key={m.id}
                    mod={m}
                    width={colWidth}
                    owned={owned.get(m.id)}
                    downloads={stats[m.id]?.downloads ?? m.stats.downloads}
                    game={showGame ? gameOf(m.game_id) : null}
                    onOpen={open}
                    onDownload={download}
                  />
                ))
              : Array.from({ length: cols }, (_, i) => (
                  // biome-ignore lint/suspicious/noArrayIndexKey: placeholders have no identity
                  <div key={i} className="card mod skeleton" aria-hidden="true">
                    <div className="skel-art" />
                    <div className="mod-body">
                      <span className="skel-line" />
                      <span className="skel-line short" />
                    </div>
                  </div>
                ))}
          </div>
        );
      })}
    </div>
  );
}

/** One card; memoized, so a re-render of the grid redraws only the cards whose data changed. */
const StoreCard = memo(function StoreCard({
  mod,
  width,
  owned,
  downloads,
  game,
  onOpen,
  onDownload,
}: {
  mod: ModSummary;
  width: number;
  owned: Owned | undefined;
  downloads: number | null;
  game: GameInfo | null;
  onOpen: (modId: number) => void;
  onDownload: (mod: { id: number; name: string }) => void;
}) {
  const sensitive = isSensitive(mod);
  const label = mod.category?.name ?? mod.root_category?.name;
  return (
    <article className="card mod store-card">
      <button type="button" className="card-hit" aria-label={`Open ${mod.name}`} onClick={() => onOpen(mod.id)} />
      <Preview image={mod.images[0]} width={width} sensitive={sensitive} alt={mod.name}>
        <div className="badges">
          <div className="badges-l">{label && <Badge tone="art">{label}</Badge>}</div>
          <div className="badges-r">
            {game && <Badge tone="game">{game.short_name}</Badge>}
            {sensitive && <Badge tone="art">Sensitive</Badge>}
            {owned?.update ? (
              <Badge tone="yellow">Update</Badge>
            ) : owned?.conflict ? (
              <Badge tone="warn">Conflict</Badge>
            ) : owned ? (
              <Badge tone={owned.on ? "ok" : "quiet"}>{owned.on ? "Installed" : "Off"}</Badge>
            ) : null}
          </div>
        </div>
      </Preview>
      <div className="mod-body">
        <h3 className="mod-title">{mod.name}</h3>
        <div className="author">
          {mod.submitter.avatar_url ? (
            <img className="av" src={mod.submitter.avatar_url} alt="" loading="lazy" />
          ) : (
            <span className="av" aria-hidden="true">
              {mod.submitter.name.slice(0, 1).toUpperCase()}
            </span>
          )}
          <span className="truncate">{mod.submitter.name}</span>
        </div>
        <div className="mod-foot">
          <div className="stats">
            <span title="Likes">
              <Icon name="heart" size={14} stroke={2.4} />
              {compact(mod.stats.likes)}
            </span>
            <span title="Views">
              <Icon name="eye" size={14} stroke={2.4} />
              {compact(mod.stats.views)}
            </span>
            <span title="Downloads">
              <Icon name="download" size={14} stroke={2.4} />
              {downloads !== null ? (
                compact(downloads)
              ) : (
                <span className="shimmer" role="img" aria-label="Loading downloads" />
              )}
            </span>
          </div>
          <button
            type="button"
            className={`dl-btn${owned ? " is-done" : ""}`}
            aria-label={
              owned
                ? owned.versions
                  ? `Add versions of ${mod.name}`
                  : `Installed; settings for ${mod.name}`
                : `Download ${mod.name}`
            }
            title={owned ? (owned.versions ? "Add versions" : "Installed: open its settings") : "Download"}
            onClick={() => onDownload({ id: mod.id, name: mod.name })}
          >
            <Icon name={owned ? "check" : "download"} size={18} stroke={2.6} />
          </button>
        </div>
      </div>
    </article>
  );
});

/** A callback that keeps its identity across renders and always calls the latest `fn`. */
function useStable<A extends unknown[]>(fn: (...args: A) => void): (...args: A) => void {
  const latest = useRef(fn);
  latest.current = fn;
  return useCallback((...args: A) => latest.current(...args), []);
}
