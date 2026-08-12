import type {
  CuttingToolDraft,
  ToolLibraryState,
} from "../../shared/tooling";

export interface ToolLibraryGateway {
  load(): Promise<ToolLibraryState>;
  create(draft: CuttingToolDraft): Promise<ToolLibraryState>;
  update(toolId: string, draft: CuttingToolDraft): Promise<ToolLibraryState>;
  delete(toolId: string): Promise<ToolLibraryState>;
  restorePresets(): Promise<ToolLibraryState>;
}
