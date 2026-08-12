import { useSyncExternalStore } from "react";

import type {
  UiExtensionRegistry,
  UiHostContext,
  UiSlotId,
} from "./UiExtensionRegistry";
import { PluginUiErrorBoundary } from "./PluginUiErrorBoundary";

interface UiExtensionSlotProps {
  registry: UiExtensionRegistry;
  slot: UiSlotId;
  context?: UiHostContext;
  onExtensionError?: (contributionId: string, error: unknown) => void;
}

export function UiExtensionSlot({
  registry,
  slot,
  context,
  onExtensionError,
}: UiExtensionSlotProps) {
  useSyncExternalStore(
    registry.subscribe,
    registry.getSnapshot,
    registry.getSnapshot,
  );

  return (
    <>
      {registry.list(slot).map((contribution) => (
        <PluginUiErrorBoundary
          contributionId={contribution.id}
          key={contribution.id}
          onError={onExtensionError}
        >
          {contribution.extension.kind === "global"
            ? contribution.extension.render()
            : context
              ? contribution.extension.render(context)
              : null}
        </PluginUiErrorBoundary>
      ))}
    </>
  );
}
