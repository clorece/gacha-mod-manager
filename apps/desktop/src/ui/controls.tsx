// SPDX-License-Identifier: MIT OR Apache-2.0
// Form controls: switch, checkbox, radio, segmented control. Checkbox and radio wrap a real
// input so the keyboard and forms behave natively; the switch is a button with role=switch.

import type { ReactNode } from "react";
import { Icon } from "./Icon";

export function Switch({
  checked,
  onChange,
  label,
  disabled = false,
  showState = false,
}: {
  checked: boolean;
  onChange: (next: boolean) => void;
  /** What is switched, for screen readers ("Casual Velina"). */
  label: string;
  disabled?: boolean;
  /** Also show "On" / "Off" before the switch: state never by color alone. */
  showState?: boolean;
}) {
  return (
    <>
      {showState && (
        <span className={`switch-label ${checked ? "state-on" : "state-off"}`} aria-hidden="true">
          {checked ? "On" : "Off"}
        </span>
      )}
      <button
        type="button"
        role="switch"
        aria-checked={checked}
        aria-label={label}
        disabled={disabled}
        className="switch"
        onClick={() => onChange(!checked)}
      >
        <span className="switch-knob" />
      </button>
    </>
  );
}

type ChoiceProps = {
  checked: boolean;
  onChange: (checked: boolean) => void;
  disabled?: boolean;
  name?: string;
  className?: string;
  children?: ReactNode;
};

export function Checkbox({ checked, onChange, disabled, name, className, children }: ChoiceProps) {
  return (
    <label className={`choice choice-check ${className ?? ""}`}>
      <input
        type="checkbox"
        checked={checked}
        disabled={disabled}
        name={name}
        onChange={(e) => onChange(e.target.checked)}
      />
      <span className="choice-box">
        <Icon name="check" size={12} stroke={4} />
      </span>
      {children}
    </label>
  );
}

export function Radio({ checked, onChange, disabled, name, className, children }: ChoiceProps) {
  return (
    <label className={`choice choice-radio ${className ?? ""}`}>
      <input
        type="radio"
        checked={checked}
        disabled={disabled}
        name={name}
        onChange={(e) => onChange(e.target.checked)}
      />
      <span className="choice-box" />
      {children}
    </label>
  );
}

export function Segmented<T extends string>({
  label,
  options,
  value,
  onChange,
}: {
  label: string;
  options: { value: T; label: string }[];
  value: T;
  onChange: (value: T) => void;
}) {
  return (
    <fieldset className="seg" aria-label={label}>
      {options.map((o) => (
        <button key={o.value} type="button" aria-pressed={o.value === value} onClick={() => onChange(o.value)}>
          {o.label}
        </button>
      ))}
    </fieldset>
  );
}
