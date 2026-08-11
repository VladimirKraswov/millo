import { Fragment, useSyncExternalStore } from "react";

import type {
  UiExtensionRegistry,
  UiHostContext,
  UiSlotId,
} from "./UiExtensionRegistry";

interface UiExtensionSlotProps {
  registry: UiExtensionRegistry;
  slot: UiSlotId;
  context: UiHostContext;
}

export function UiExtensionSlot({
  registry,
  slot,
  context,
}: UiExtensionSlotProps) {
  useSyncExternalStore(
    registry.subscribe,
    registry.getSnapshot,
    registry.getSnapshot,
  );

  return (
    <>
      {registry.list(slot).map((contribution) => (
        <Fragment key={contribution.id}>
          {contribution.extension(context)}
        </Fragment>
      ))}
    </>
  );
}
