import type { GeneratedJob, PublishedJob } from "../../shared/jobs";

export class GeneratedJobStore {
  private readonly listeners = new Set<() => void>();
  private snapshot: PublishedJob | undefined;
  private sequence = 0;

  readonly current = (): PublishedJob | undefined => this.snapshot;

  readonly subscribe = (listener: () => void): (() => void) => {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  };

  publish(job: GeneratedJob): PublishedJob {
    this.sequence += 1;
    this.snapshot = Object.freeze({ sequence: this.sequence, job });
    for (const listener of this.listeners) listener();
    return this.snapshot;
  }
}
