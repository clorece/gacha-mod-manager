// SPDX-License-Identifier: MIT OR Apache-2.0
// A pill dropdown: single choice (Category, Sort) or several with checkboxes (Characters,
// where nothing ticked means all). Closes on Escape, outside clicks and picking (single);
// arrow keys move between options.

import { type KeyboardEvent, type ReactNode, useEffect, useId, useRef, useState } from "react";
import { Icon } from "./Icon";

export type Option<T> = { value: T; label: string; icon?: ReactNode };

type Common<T> = {
  /** The muted key before the value ("Category"). */
  label: string;
  options: Option<T>[];
  disabled?: boolean;
  /** Shown as the tooltip when disabled, so the reason is never hidden. */
  disabledReason?: string;
};

type Single<T> = Common<T> & { multiple?: false; value: T; onChange: (value: T) => void };
type Multi<T> = Common<T> & {
  multiple: true;
  value: T[];
  onChange: (value: T[]) => void;
  /** The "nothing ticked" option ("All characters"). */
  allLabel: string;
};

export function Dropdown<T extends string | number>(props: Single<T> | Multi<T>) {
  const [open, setOpen] = useState(false);
  const root = useRef<HTMLDivElement>(null);
  const button = useRef<HTMLButtonElement>(null);
  const menuId = useId();

  useEffect(() => {
    if (!open) return;
    const onDown = (e: MouseEvent) => {
      if (!root.current?.contains(e.target as Node)) setOpen(false);
    };
    document.addEventListener("mousedown", onDown);
    return () => document.removeEventListener("mousedown", onDown);
  }, [open]);

  useEffect(() => {
    if (open) root.current?.querySelector<HTMLButtonElement>('[role="option"]')?.focus();
  }, [open]);

  const close = () => {
    setOpen(false);
    button.current?.focus();
  };

  const onMenuKey = (e: KeyboardEvent<HTMLDivElement>) => {
    const items = [...(root.current?.querySelectorAll<HTMLButtonElement>('[role="option"]') ?? [])];
    const at = items.indexOf(document.activeElement as HTMLButtonElement);
    if (e.key === "Escape") {
      e.preventDefault();
      close();
    } else if (e.key === "ArrowDown" || e.key === "ArrowUp") {
      e.preventDefault();
      const next = e.key === "ArrowDown" ? Math.min(items.length - 1, at + 1) : Math.max(0, at - 1);
      items[next]?.focus();
    } else if (e.key === "Tab") {
      setOpen(false);
    }
  };

  let summary: string;
  if (props.multiple) {
    const picked = props.options.filter((o) => props.value.includes(o.value));
    summary =
      picked.length === 0 ? props.allLabel : picked.length === 1 ? picked[0].label : `${picked.length} selected`;
  } else {
    summary = props.options.find((o) => o.value === props.value)?.label ?? "";
  }

  return (
    <div className="dd" ref={root}>
      <button
        ref={button}
        type="button"
        className="dd-btn"
        aria-haspopup="listbox"
        aria-expanded={open}
        aria-controls={open ? menuId : undefined}
        disabled={props.disabled}
        title={props.disabled ? props.disabledReason : undefined}
        onClick={() => setOpen((o) => !o)}
      >
        <span className="dd-k">{props.label}</span>
        <span>{summary}</span>
        <Icon name={open ? "chevronUp" : "chevronDown"} size={16} stroke={2.6} />
      </button>
      {open && (
        <div
          id={menuId}
          className="dd-menu"
          role="listbox"
          aria-label={props.label}
          aria-multiselectable={props.multiple || undefined}
          tabIndex={-1}
          onKeyDown={onMenuKey}
        >
          {props.multiple ? (
            <>
              <MenuOption selected={props.value.length === 0} check onPick={() => props.onChange([])}>
                {props.allLabel}
              </MenuOption>
              {props.options.map((o) => {
                const on = props.value.includes(o.value);
                return (
                  <MenuOption
                    key={o.value}
                    selected={on}
                    check
                    onPick={() =>
                      props.onChange(on ? props.value.filter((v) => v !== o.value) : [...props.value, o.value])
                    }
                  >
                    {o.icon}
                    {o.label}
                  </MenuOption>
                );
              })}
            </>
          ) : (
            props.options.map((o) => (
              <MenuOption
                key={o.value}
                selected={o.value === props.value}
                onPick={() => {
                  props.onChange(o.value);
                  close();
                }}
              >
                {o.icon}
                {o.label}
              </MenuOption>
            ))
          )}
        </div>
      )}
    </div>
  );
}

function MenuOption({
  selected,
  check = false,
  onPick,
  children,
}: {
  selected: boolean;
  check?: boolean;
  onPick: () => void;
  children: ReactNode;
}) {
  return (
    <button type="button" role="option" aria-selected={selected} className="dd-opt" onClick={onPick}>
      {check && (
        <span className="choice-check" aria-hidden="true">
          <span className={`choice-box${selected ? " is-on" : ""}`}>
            <Icon name="check" size={12} stroke={4} />
          </span>
        </span>
      )}
      {children}
    </button>
  );
}
