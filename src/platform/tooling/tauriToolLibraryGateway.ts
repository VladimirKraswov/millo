import { invoke } from "@tauri-apps/api/core";

import type { CuttingToolDraft, ToolLibraryState } from "../../shared/tooling";
import type { ToolLibraryGateway } from "./ToolLibraryGateway";

export const tauriToolLibraryGateway: ToolLibraryGateway = Object.freeze({
  load: () => invoke<ToolLibraryState>("tool_library"),
  create: (draft: CuttingToolDraft) =>
    invoke<ToolLibraryState>("create_cutting_tool", { draft }),
  update: (toolId: string, draft: CuttingToolDraft) =>
    invoke<ToolLibraryState>("update_cutting_tool", { toolId, draft }),
  delete: (toolId: string) =>
    invoke<ToolLibraryState>("delete_cutting_tool", { toolId }),
  restorePresets: () => invoke<ToolLibraryState>("restore_cutting_tool_presets"),
});
