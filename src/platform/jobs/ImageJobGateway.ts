import type {
  GeneratedJob,
  GeneratedGcodeSaveOutcome,
  GeneratedImageJob,
  GeneratedPcbJob,
  GeneratedSurfacingJob,
  ImageJobRequest,
  PcbInspectRequest,
  PcbInspection,
  PcbJobRequest,
  SurfacingJobRequest,
} from "../../shared/jobs";

export interface ImageJobGateway {
  generate(request: ImageJobRequest): Promise<GeneratedImageJob>;
  generateSurfacing(request: SurfacingJobRequest): Promise<GeneratedSurfacingJob>;
  inspectPcb(request: PcbInspectRequest): Promise<PcbInspection>;
  generatePcb(request: PcbJobRequest): Promise<GeneratedPcbJob>;
  save(job: GeneratedJob): Promise<GeneratedGcodeSaveOutcome | undefined>;
}
