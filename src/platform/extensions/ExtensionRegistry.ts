export interface ExtensionContribution<TSlot extends string, TExtension> {
  readonly id: string;
  readonly owner: string;
  readonly slot: TSlot;
  readonly order?: number;
  readonly replaces?: readonly string[];
  readonly extension: TExtension;
}

export interface ExtensionRegistration {
  dispose(): void;
}

export class ExtensionRegistry<TSlot extends string, TExtension> {
  private readonly contributions = new Map<
    string,
    ExtensionContribution<TSlot, TExtension>
  >();
  private readonly listeners = new Set<() => void>();
  private revision = 0;

  readonly getSnapshot = (): number => this.revision;

  readonly subscribe = (listener: () => void): (() => void) => {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  };

  register(
    contribution: ExtensionContribution<TSlot, TExtension>,
  ): ExtensionRegistration {
    this.validate(contribution);
    if (this.contributions.has(contribution.id)) {
      throw new Error(`extension contribution already registered: ${contribution.id}`);
    }

    const stored = Object.freeze({
      ...contribution,
      replaces: contribution.replaces
        ? Object.freeze([...contribution.replaces])
        : undefined,
    });
    this.contributions.set(stored.id, stored);
    this.changed();
    let active = true;
    return {
      dispose: () => {
        if (!active) return;
        active = false;
        this.unregister(stored.id);
      },
    };
  }

  unregister(id: string): boolean {
    const removed = this.contributions.delete(id);
    if (removed) this.changed();
    return removed;
  }

  unregisterOwner(owner: string): number {
    let removed = 0;
    for (const [id, contribution] of this.contributions) {
      if (contribution.owner === owner) {
        this.contributions.delete(id);
        removed += 1;
      }
    }
    if (removed > 0) this.changed();
    return removed;
  }

  list(slot: TSlot): readonly ExtensionContribution<TSlot, TExtension>[] {
    const matching = [...this.contributions.values()].filter(
      (contribution) => contribution.slot === slot,
    );
    const replaced = new Set(
      matching.flatMap((contribution) => contribution.replaces ?? []),
    );

    return matching
      .filter((contribution) => !replaced.has(contribution.id))
      .sort(
        (left, right) =>
          (left.order ?? 0) - (right.order ?? 0) ||
          left.id.localeCompare(right.id),
      );
  }

  private validate(
    contribution: ExtensionContribution<TSlot, TExtension>,
  ): void {
    if (!contribution.id.trim()) {
      throw new Error("extension contribution id must not be empty");
    }
    if (!contribution.owner.trim()) {
      throw new Error("extension contribution owner must not be empty");
    }
    if (!contribution.slot.trim()) {
      throw new Error("extension contribution slot must not be empty");
    }
    if (
      contribution.order !== undefined &&
      !Number.isFinite(contribution.order)
    ) {
      throw new Error("extension contribution order must be finite");
    }
    if (contribution.replaces?.includes(contribution.id)) {
      throw new Error("extension contribution cannot replace itself");
    }
  }

  private changed(): void {
    this.revision += 1;
    for (const listener of this.listeners) listener();
  }
}
