// SPDX-License-Identifier: MIT OR Apache-2.0
// "Select Game": the selected game's banner as a button; it opens a list of banners (All
// games, then each game with its mod count). The choice scopes the Library and the Store.

import { useEffect, useRef, useState } from "react";
import { plural } from "../format";
import { ALL, bannerClass, type Scope } from "../games";
import { Icon } from "../ui";

export type PickerGame = { id: string; name: string; short: string; mods: number };

export function GamePicker({
  games,
  scope,
  onScope,
}: {
  games: PickerGame[];
  scope: Scope;
  onScope: (scope: Scope) => void;
}) {
  const [open, setOpen] = useState(false);
  const root = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!open) return;
    const onDown = (e: MouseEvent) => {
      if (!root.current?.contains(e.target as Node)) setOpen(false);
    };
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") setOpen(false);
    };
    document.addEventListener("mousedown", onDown);
    document.addEventListener("keydown", onKey);
    return () => {
      document.removeEventListener("mousedown", onDown);
      document.removeEventListener("keydown", onKey);
    };
  }, [open]);

  const options: PickerGame[] = [
    { id: ALL, name: "All games", short: "ALL", mods: games.reduce((n, g) => n + g.mods, 0) },
    ...games,
  ];
  const current = options.find((o) => o.id === scope) ?? options[0];

  return (
    <div className="picker" ref={root}>
      <button
        type="button"
        className={`${bannerClass(current.id)} picker-btn`}
        data-mark={current.short}
        aria-expanded={open}
        aria-label={`Select game: ${current.name}`}
        onClick={() => setOpen((o) => !o)}
      >
        <span className="fade" />
        <span className="art-chip">Game</span>
        <span className="banner-title">{current.name}</span>
        <span className="picker-chev">
          <Icon name={open ? "chevronUp" : "chevronDown"} stroke={2.5} />
        </span>
      </button>
      {open && (
        <div className="picker-list">
          {options.map((o) => (
            <button
              key={o.id}
              type="button"
              className={`${bannerClass(o.id)} picker-opt`}
              data-mark={o.short}
              aria-current={o.id === current.id ? "true" : undefined}
              onClick={() => {
                onScope(o.id);
                setOpen(false);
              }}
            >
              <span className="fade" />
              <span className="banner-title">{o.name}</span>
              <span className="art-chip">{plural(o.mods, "mod")}</span>
            </button>
          ))}
        </div>
      )}
    </div>
  );
}
