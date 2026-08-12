import type {
  GeneratedGcodeSaveOutcome,
  GeneratedImageJob,
  ImageJobRequest,
} from "../../shared/jobs";
import type { ImageJobGateway } from "./ImageJobGateway";
import { GeneratedJobStore } from "./GeneratedJobStore";

export interface JobCreationCapability {
  generateImage(request: ImageJobRequest): Promise<GeneratedImageJob>;
  open(job: GeneratedImageJob): void;
  save(job: GeneratedImageJob): Promise<GeneratedGcodeSaveOutcome | undefined>;
}

export class JobCreationService implements JobCreationCapability {
  private readonly issuedJobs = new WeakSet<object>();

  constructor(
    private readonly gateway: ImageJobGateway,
    private readonly store: GeneratedJobStore,
  ) {}

  async generateImage(request: ImageJobRequest): Promise<GeneratedImageJob> {
    const result = deepFreeze(await this.gateway.generate(request));
    this.issuedJobs.add(result);
    return result;
  }

  open(job: GeneratedImageJob): void {
    this.assertIssued(job);
    this.store.publish(job);
  }

  save(job: GeneratedImageJob): Promise<GeneratedGcodeSaveOutcome | undefined> {
    this.assertIssued(job);
    return this.gateway.save(job);
  }

  private assertIssued(job: GeneratedImageJob): void {
    if (!this.issuedJobs.has(job)) {
      throw new Error("job was not issued by the Millo generation core");
    }
  }
}

function deepFreeze<T>(value: T): T {
  if (typeof value !== "object" || value === null || Object.isFrozen(value)) {
    return value;
  }
  Object.freeze(value);
  for (const child of Object.values(value)) deepFreeze(child);
  return value;
}
