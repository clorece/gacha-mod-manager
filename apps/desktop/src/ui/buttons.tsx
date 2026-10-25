// SPDX-License-Identifier: MIT OR Apache-2.0
// Buttons: text buttons in five styles and round icon buttons (docs/08, "Tokens"). Every
// style sets its own colors; the base `:where(button)` rule only sets the font.

import type { ButtonHTMLAttributes } from "react";
import { Icon, type IconName } from "./Icon";

type Native = Omit<ButtonHTMLAttributes<HTMLButtonElement>, "type">;

export type ButtonVariant = "default" | "primary" | "launch" | "danger" | "link";

export function Button({
  variant = "default",
  size = "md",
  block = false,
  icon,
  type = "button",
  className,
  children,
  ...rest
}: Native & {
  variant?: ButtonVariant;
  size?: "md" | "sm";
  block?: boolean;
  icon?: IconName;
  type?: "button" | "submit";
}) {
  const classes = [
    "btn",
    variant !== "default" && `btn-${variant}`,
    size === "sm" && "btn-sm",
    block && "btn-block",
    className,
  ]
    .filter(Boolean)
    .join(" ");
  return (
    <button type={type} className={classes} {...rest}>
      {icon && <Icon name={icon} size={variant === "launch" ? 18 : 16} stroke={2.6} filled={icon === "play"} />}
      {children}
    </button>
  );
}

export function IconButton({
  icon,
  label,
  variant = "default",
  size = "md",
  className,
  ...rest
}: Native & {
  icon: IconName;
  /** Read by screen readers and shown as the tooltip. */
  label: string;
  /** `art` sits on artwork: dark scrim, white icon. */
  variant?: "default" | "primary" | "art";
  size?: "md" | "sm";
}) {
  const classes = [
    "icon-btn",
    variant !== "default" && `icon-btn-${variant}`,
    size === "sm" && "icon-btn-sm",
    className,
  ]
    .filter(Boolean)
    .join(" ");
  return (
    <button type="button" aria-label={label} title={label} className={classes} {...rest}>
      <Icon name={icon} size={size === "sm" ? 16 : 18} stroke={2.6} />
    </button>
  );
}
