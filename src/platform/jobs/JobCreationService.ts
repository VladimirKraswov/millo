import type {
  GeneratedJob,
  GeneratedGcodeSaveOutcome,
  GeneratedImageJob,
  GeneratedSurfacingJob,
  ImageJobRequest,
  SurfacingJobRequest,
} from "../../shared/jobs";
import type { ImageJobGateway } from "./ImageJobGateway";
import { GeneratedJobStore } from "./GeneratedJobStore";

export interface JobCreationCapability {
  generateImage(request: ImageJobRequest): Promise<GeneratedImageJob>;
  generateSurfacing(request: SurfacingJobRequest): Promise<GeneratedSurfacingJob>;
  open(job: GeneratedJob): void;
  save(job: GeneratedJob): Promise<GeneratedGcodeSaveOutcome | undefined>;
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

  async generateSurfacing(request: SurfacingJobRequest): Promise<GeneratedSurfacingJob> {
    const result = deepFreeze(await this.gateway.generateSurfacing(request));
    this.issuedJobs.add(result);
    return result;
  }

  open(job: GeneratedJob): void {
    this.assertIssued(job);
    this.store.publish(job);
  }

  save(job: GeneratedJob): Promise<GeneratedGcodeSaveOutcome | undefined> {
    this.assertIssued(job);
    return this.gateway.save(job);
  }

  private assertIssued(job: GeneratedJob): void {
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
