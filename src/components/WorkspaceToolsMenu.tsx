import { Plus, ChevronDown } from "lucide-react";
import { useEffect, useRef, useState, useSyncExternalStore } from "react";

import { UiExtensionSlot } from "../platform/extensions/UiExtensionSlot";
import {
  uiSlots,
  type UiExtensionRegistry,
} from "../platform/extensions/UiExtensionRegistry";

export function WorkspaceToolsMenu({
  registry,
}: {
  readonly registry: UiExtensionRegistry;
}) {
  const rootRef = useRef<HTMLDivElement>(null);
  const [open, setOpen] = useState(false);
  useSyncExternalStore(
    registry.subscribe,
    registry.getSnapshot,
    registry.getSnapshot,
  );
  const count = registry.list(uiSlots.workspaceTools).length;

  useEffect(() => {
    if (!open) return;
    const close = (event: PointerEvent) => {
      if (!rootRef.current?.contains(event.target as Node)) setOpen(false);
    };
    const closeOnEscape = (event: KeyboardEvent) => {
      if (event.key === "Escape") setOpen(false);
    };
    window.addEventListener("pointerdown", close);
    window.addEventListener("keydown", closeOnEscape);
    return () => {
      window.removeEventListener("pointerdown", close);
      window.removeEventListener("keydown", closeOnEscape);
    };
  }, [open]);

  if (count === 0) return null;

  return (
    <div className="workspace-tools-menu" ref={rootRef}>
      <button
        aria-expanded={open}
        aria-haspopup="menu"
        className="workspace-tools-menu-trigger"
        onClick={() => setOpen((current) => !current)}
        type="button"
      >
        <Plus aria-hidden="true" size={15} />
        <span>Создать</span>
        <ChevronDown aria-hidden="true" size={13} />
      </button>
      <div
        aria-label="Создать задание"
        className="workspace-tools-menu-popover"
        hidden={!open}
        onClick={() => setOpen(false)}
        role="menu"
      >
        <span>Новое задание</span>
        <UiExtensionSlot registry={registry} slot={uiSlots.workspaceTools} />
      </div>
    </div>
  );
}
