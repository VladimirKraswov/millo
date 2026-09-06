import type {
  CuttingToolDraft,
  ToolLibraryState,
} from "../../shared/tooling";
import { emptyToolLibrary } from "../../shared/tooling";
import { notifyListeners } from "../state/notifyListeners";
import type { ToolLibraryGateway } from "./ToolLibraryGateway";

export type ToolLibraryListener = (state: ToolLibraryState) => void;

export class ToolLibraryService {
  private snapshot: ToolLibraryState = emptyToolLibrary;
  private initialized = false;
  private readonly listeners = new Set<ToolLibraryListener>();

  constructor(
    private readonly gateway: ToolLibraryGateway,
    private readonly onListenerError: (error: unknown) => void = console.error,
  ) {}

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
    if (this.initialized && state.revision <= this.snapshot.revision) return this.snapshot;
    const current = deepFreeze(structuredClone(state));
    this.initialized = true;
    this.snapshot = current;
    notifyListeners(this.listeners, current, this.onListenerError);
    return current;
  }
}

function deepFreeze<T>(value: T): T {
  if (typeof value !== "object" || value === null) return value;
  for (const child of Object.values(value)) deepFreeze(child);
  Object.freeze(value);
  return value;
}
