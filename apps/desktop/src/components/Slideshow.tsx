// SPDX-License-Identifier: MIT OR Apache-2.0
// The mod page's slideshow (docs/07, "Mod page"), built by hand: the current preview large,
// arrow buttons and ←/→ keys, a thumbnail strip, and a click opening a full-window lightbox
// on a native <dialog> (focus trap, Escape). One image: no controls. None: a placeholder.
// Every image is a `gmmimg` URL from Rust.

import { type KeyboardEvent, useEffect, useRef, useState } from "react";
import type { Image } from "../api";
import { imageAt } from "../catalog";
import { IconButton } from "../ui";
import { Preview } from "./Preview";

export function Slideshow({ images, name, sensitive }: { images: Image[]; name: string; sensitive: boolean }) {
  const [index, setIndex] = useState(0);
  const [zoom, setZoom] = useState(false);
  const thumbs = useRef<HTMLDivElement>(null);
  const many = images.length > 1;
  const at = Math.min(index, Math.max(images.length - 1, 0));
  const step = (d: number) => setIndex((at + d + images.length) % images.length);

  // Keep the current thumbnail in view.
  useEffect(() => {
    const el = thumbs.current?.children[at] as HTMLElement | undefined;
    el?.scrollIntoView({ block: "nearest", inline: "nearest" });
  }, [at]);

  const onKey = (e: KeyboardEvent) => {
    if (!many) return;
    if (e.key === "ArrowLeft") {
      e.preventDefault();
      step(-1);
    } else if (e.key === "ArrowRight") {
      e.preventDefault();
      step(1);
    }
  };

  return (
    // biome-ignore lint/a11y/noStaticElementInteractions: arrow keys while focus is inside the slideshow
    <div className="slides" onKeyDown={onKey}>
      <div className="slide-stage">
        <button
          type="button"
          className="slide-main"
          onClick={() => images.length > 0 && setZoom(true)}
          aria-label={images.length > 0 ? `Enlarge image ${at + 1} of ${images.length}` : `${name}: no images`}
          disabled={images.length === 0}
        >
          <Preview image={images[at]} width={900} sensitive={sensitive} alt={name} eager />
        </button>
        {many && (
          <>
            <span className="slide-nav slide-prev">
              <IconButton icon="chevronLeft" label="Previous image" variant="art" onClick={() => step(-1)} />
            </span>
            <span className="slide-nav slide-next">
              <IconButton icon="chevronRight" label="Next image" variant="art" onClick={() => step(1)} />
            </span>
            <span className="slide-count">
              {at + 1} / {images.length}
            </span>
          </>
        )}
      </div>
      {many && (
        <div className="slide-thumbs" ref={thumbs}>
          {images.map((img, i) => (
            <button
              // biome-ignore lint/suspicious/noArrayIndexKey: an image's place in GameBanana's list is its identity here
              key={i}
              type="button"
              className={`slide-thumb${i === at ? " is-current" : ""}`}
              aria-label={`Image ${i + 1}`}
              aria-current={i === at ? "true" : undefined}
              onClick={() => setIndex(i)}
            >
              <Preview image={img} width={120} sensitive={sensitive} alt="" />
            </button>
          ))}
        </div>
      )}
      {zoom && images[at] && (
        <Lightbox images={images} index={at} name={name} onIndex={setIndex} onClose={() => setZoom(false)} />
      )}
    </div>
  );
}

function Lightbox({
  images,
  index,
  name,
  onIndex,
  onClose,
}: {
  images: Image[];
  index: number;
  name: string;
  onIndex: (i: number) => void;
  onClose: () => void;
}) {
  const ref = useRef<HTMLDialogElement>(null);
  useEffect(() => {
    const dialog = ref.current;
    if (dialog && !dialog.open) dialog.showModal();
    return () => dialog?.close();
  }, []);
  const many = images.length > 1;
  const step = (d: number) => onIndex((index + d + images.length) % images.length);
  return (
    <dialog
      ref={ref}
      className="lightbox"
      aria-label={`${name}, image ${index + 1} of ${images.length}`}
      onCancel={(e) => {
        e.preventDefault();
        onClose();
      }}
      onKeyDown={(e) => {
        if (!many) return;
        if (e.key === "ArrowLeft") step(-1);
        if (e.key === "ArrowRight") step(1);
      }}
      onMouseDown={(e) => {
        if (e.target === e.currentTarget) onClose();
      }}
    >
      <img
        className="lightbox-img"
        src={imageAt(images[index], 4000)}
        alt={`${name}, ${index + 1} of ${images.length}`}
      />
      <span className="lightbox-close">
        <IconButton icon="close" label="Close" variant="art" onClick={onClose} />
      </span>
      {many && (
        <>
          <span className="slide-nav slide-prev">
            <IconButton icon="chevronLeft" label="Previous image" variant="art" onClick={() => step(-1)} />
          </span>
          <span className="slide-nav slide-next">
            <IconButton icon="chevronRight" label="Next image" variant="art" onClick={() => step(1)} />
          </span>
          <span className="slide-count">
            {index + 1} / {images.length}
          </span>
        </>
      )}
    </dialog>
  );
}
