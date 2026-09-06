import type { GeneratedSketchJob, SketchJobRequest } from "../../shared/sketch";
import type {
  GeneratedJob,
  GeneratedGcodeSaveOutcome,
  GeneratedImageJob,
  GeneratedPcbJob,
  GeneratedSurfacingJob,
  ImageJobRequest,
  JobToolAssignment,
  PcbInspectRequest,
  PcbInspection,
  PcbJobRequest,
  SurfacingJobRequest,
} from "../../shared/jobs";
import type { ImageJobGateway } from "./ImageJobGateway";
import { GeneratedJobStore } from "./GeneratedJobStore";

export interface JobCreationCapability {
  generateSketch(request: SketchJobRequest): Promise<GeneratedSketchJob>;
  saveSketchProject(request: SketchJobRequest): Promise<GeneratedGcodeSaveOutcome | undefined>;
  generateImage(request: ImageJobRequest): Promise<GeneratedImageJob>;
  generateSurfacing(request: SurfacingJobRequest): Promise<GeneratedSurfacingJob>;
  inspectPcb(request: PcbInspectRequest): Promise<PcbInspection>;
  generatePcb(request: PcbJobRequest): Promise<GeneratedPcbJob>;
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

  saveSketchProject(request: SketchJobRequest): Promise<GeneratedGcodeSaveOutcome | undefined> {
    return this.gateway.saveSketchProject(request);
  }

  async generateSketch(request: SketchJobRequest): Promise<GeneratedSketchJob> {
    const generated = await this.gateway.generateSketch(request);
    const result = deepFreeze({
      ...generated,
      toolAssignments: uniqueToolAssignments(generated.summary.operations),
    });
    this.issuedJobs.add(result);
    return result;
  }

  async generateSurfacing(request: SurfacingJobRequest): Promise<GeneratedSurfacingJob> {
    const toolId = request.toolId;
    const generated = await this.gateway.generateSurfacing(request);
    const result = deepFreeze({
      ...generated,
      toolAssignments: [{ toolNumber: 1, toolId }],
    });
    this.issuedJobs.add(result);
    return result;
  }

  async inspectPcb(request: PcbInspectRequest): Promise<PcbInspection> {
    return deepFreeze(await this.gateway.inspectPcb(request));
  }

  async generatePcb(request: PcbJobRequest): Promise<GeneratedPcbJob> {
    const generated = await this.gateway.generatePcb(request);
    const result = deepFreeze({
      ...generated,
      toolAssignments: uniqueToolAssignments(generated.summary.operations),
    });
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

function uniqueToolAssignments(operations: readonly JobToolAssignment[]): JobToolAssignment[] {
  const assignments = new Map(operations.map((operation) => [operation.toolNumber, operation.toolId]));
  return [...assignments].map(([toolNumber, toolId]) => ({ toolNumber, toolId }));
}
