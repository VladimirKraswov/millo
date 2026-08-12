import type {
  CuttingToolDraft,
  ToolLibraryState,
} from "../../shared/tooling";
import { emptyToolLibrary } from "../../shared/tooling";
import type { ToolLibraryGateway } from "./ToolLibraryGateway";

export type ToolLibraryListener = (state: ToolLibraryState) => void;

export class ToolLibraryService {
  private snapshot: ToolLibraryState = emptyToolLibrary;
  private readonly listeners = new Set<ToolLibraryListener>();

  constructor(private readonly gateway: ToolLibraryGateway) {}

  readonly current = (): ToolLibraryState => this.snapshot;

  readonly subscribe = (listener: ToolLibraryListener): (() => void) => {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  };

  async initialize(): Promise<ToolLibraryState> {
    return this.publish(await this.gateway.load());
  }

  async create(draft: CuttingToolDraft): Promise<ToolLibraryState> {
    return this.publish(await this.gateway.create(draft));
  }

  async update(toolId: string, draft: CuttingToolDraft): Promise<ToolLibraryState> {
    return this.publish(await this.gateway.update(toolId, draft));
  }

  async delete(toolId: string): Promise<ToolLibraryState> {
    return this.publish(await this.gateway.delete(toolId));
  }

  async restorePresets(): Promise<ToolLibraryState> {
    return this.publish(await this.gateway.restorePresets());
  }

  private publish(state: ToolLibraryState): ToolLibraryState {
    this.snapshot = deepFreeze({
      tools: [...state.tools],
      revision: state.revision,
    });
    for (const listener of this.listeners) listener(this.snapshot);
    return this.snapshot;
  }
}

function deepFreeze<T>(value: T): T {
  if (typeof value !== "object" || value === null || Object.isFrozen(value)) return value;
  Object.freeze(value);
  for (const child of Object.values(value)) deepFreeze(child);
  return value;
}
