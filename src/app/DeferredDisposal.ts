export class DeferredDisposal {
  private disposed = false;
  private generation = 0;

  constructor(
    private readonly dispose: () => void | Promise<void>,
    private readonly onError: (error: unknown) => void = () => undefined,
  ) {}

  mount(): () => void {
    if (this.disposed) {
      throw new Error("cannot mount a disposed lifecycle");
    }
    const generation = ++this.generation;
    return () => {
      queueMicrotask(() => {
        if (this.disposed || generation !== this.generation) return;
        this.disposed = true;
        try {
          void Promise.resolve(this.dispose()).catch(this.onError);
        } catch (error) {
          this.onError(error);
        }
      });
    };
  }
}
