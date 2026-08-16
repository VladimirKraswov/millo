import { useCallback, useSyncExternalStore } from "react";

import type { PluginToolsCapability } from "../plugin-sdk";
import type { ToolLibraryState } from "../shared/tooling";

export function usePluginToolLibrary(
  tools: PluginToolsCapability,
): ToolLibraryState {
  const subscribe = useCallback(
    (notify: () => void) => tools.subscribe(notify),
    [tools],
  );
  return useSyncExternalStore(subscribe, tools.current, tools.current);
}
