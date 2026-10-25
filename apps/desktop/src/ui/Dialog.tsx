// SPDX-License-Identifier: MIT OR Apache-2.0
// Dialog and side panel, both on the native <dialog> element: it traps focus, closes on
// Escape and returns focus to what opened it. Render them only while open
// (`{open && <Dialog …/>}`); clicking the backdrop closes them too.

import { type MouseEvent as ReactMouseEvent, type ReactNode, useEffect, useId, useRef } from "react";
import { IconButton } from "./buttons";

function useModal() {
  const ref = useRef<HTMLDialogElement>(null);
  useEffect(() => {
    const dialog = ref.current;
    if (dialog && !dialog.open) dialog.showModal();
    return () => dialog?.close();
  }, []);
  return ref;
}

/** Closes when the press lands on the backdrop (the dialog element itself, outside its box). */
function backdropClose(onClose: () => void) {
  return (e: ReactMouseEvent<HTMLDialogElement>) => {
    if (e.target !== e.currentTarget) return;
    const box = e.currentTarget.getBoundingClientRect();
    const inside = e.clientX >= box.left && e.clientX <= box.right && e.clientY >= box.top && e.clientY <= box.bottom;
    if (!inside) onClose();
  };
}

export function Dialog({
  title,
  eyebrow,
  media,
  onClose,
  footer,
  children,
}: {
  title: ReactNode;
  /** Small accent line above the title ("Install"). */
  eyebrow?: string;
  /** A thumbnail before the title. */
  media?: ReactNode;
  onClose: () => void;
  /** The buttons; the primary one last. */
  footer?: ReactNode;
  children: ReactNode;
}) {
  const ref = useModal();
  const titleId = useId();
  return (
    <dialog
      ref={ref}
      className="modal"
      aria-labelledby={titleId}
      onCancel={(e) => {
        e.preventDefault();
        onClose();
      }}
      onMouseDown={backdropClose(onClose)}
    >
      <div className="modal-head">
        {media}
        <div className="grow">
          {eyebrow && <div className="eyebrow">{eyebrow}</div>}
          <h2 id={titleId} className="modal-title">
            {title}
          </h2>
        </div>
      </div>
      <div className="modal-body">{children}</div>
      {footer && <div className="modal-foot">{footer}</div>}
    </dialog>
  );
}

export function SidePanel({
  title,
  eyebrow,
  onClose,
  children,
}: {
  title: ReactNode;
  eyebrow?: string;
  onClose: () => void;
  children: ReactNode;
}) {
  const ref = useModal();
  const titleId = useId();
  return (
    <dialog
      ref={ref}
      className="drawer"
      aria-labelledby={titleId}
      onCancel={(e) => {
        e.preventDefault();
        onClose();
      }}
      onMouseDown={backdropClose(onClose)}
    >
      <div className="drawer-head">
        <div className="grow">
          {eyebrow && <div className="eyebrow">{eyebrow}</div>}
          <h2 id={titleId} className="modal-title">
            {title}
          </h2>
        </div>
        <IconButton icon="close" label="Close" size="sm" onClick={onClose} />
      </div>
      <div className="drawer-body">{children}</div>
    </dialog>
  );
}
