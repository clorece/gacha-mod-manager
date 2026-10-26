// SPDX-License-Identifier: MIT OR Apache-2.0
// A mod's preview image (always a `gmmimg` URL from Rust, never an outside address), with the
// sensitive-content rule: when the user shows sensitive mods and keeps "blur" on, the image
// stays blurred until the card is hovered or focused. No image: a quiet placeholder. The
// image fades in once it has loaded and been decoded, so a half-drawn picture never shows.

import { type ReactNode, useState } from "react";
import type { Image } from "../api";
import { imageAt } from "../catalog";
import { usePrefs } from "../prefs";
import { Icon } from "../ui";

export function Preview({
  image,
  width,
  sensitive,
  alt,
  className,
  eager = false,
  children,
}: {
  image: Image | undefined;
  /** The displayed width in CSS pixels; a copy twice as wide is picked for sharp screens. */
  width: number;
  sensitive: boolean;
  alt: string;
  className?: string;
  /** Above the fold (the featured banner): load first, at high priority. */
  eager?: boolean;
  /** Badges and labels over the image. */
  children?: ReactNode;
}) {
  const { prefs } = usePrefs();
  const src = imageAt(image, width * 2);
  const blur = sensitive && prefs.blur_sensitive;
  const [shown, setShown] = useState<string | null>(null);
  return (
    <div className={`preview${blur ? " is-blur" : ""}${className ? ` ${className}` : ""}`}>
      {src ? (
        <img
          className={`preview-img${shown === src ? " is-loaded" : ""}`}
          src={src}
          alt={alt}
          loading={eager ? "eager" : "lazy"}
          fetchPriority={eager ? "high" : "auto"}
          decoding="async"
          draggable={false}
          onLoad={() => setShown(src)}
        />
      ) : (
        <span className="preview-none" role="img" aria-label={`${alt}: no preview image`}>
          <Icon name="image" size={28} />
        </span>
      )}
      {children}
    </div>
  );
}
