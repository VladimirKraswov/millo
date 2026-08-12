import type {
  GeneratedGcodeSaveOutcome,
  GeneratedImageJob,
  ImageJobRequest,
} from "../../shared/jobs";

export interface ImageJobGateway {
  generate(request: ImageJobRequest): Promise<GeneratedImageJob>;
  save(job: GeneratedImageJob): Promise<GeneratedGcodeSaveOutcome | undefined>;
}
