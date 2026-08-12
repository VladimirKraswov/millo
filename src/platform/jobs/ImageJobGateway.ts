import type {
  GeneratedJob,
  GeneratedGcodeSaveOutcome,
  GeneratedImageJob,
  GeneratedSurfacingJob,
  ImageJobRequest,
  SurfacingJobRequest,
} from "../../shared/jobs";

export interface ImageJobGateway {
  generate(request: ImageJobRequest): Promise<GeneratedImageJob>;
  generateSurfacing(request: SurfacingJobRequest): Promise<GeneratedSurfacingJob>;
  save(job: GeneratedJob): Promise<GeneratedGcodeSaveOutcome | undefined>;
}
