import { invoke } from "@tauri-apps/api/core";

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
import type { ImageJobGateway } from "./ImageJobGateway";

export const tauriImageJobGateway: ImageJobGateway = {
  generate: (request: ImageJobRequest) =>
    invoke<GeneratedImageJob>("generate_image_job", { request }),
  generateSurfacing: (request: SurfacingJobRequest) =>
    invoke<GeneratedSurfacingJob>("generate_surfacing_job", { request }),
  inspectPcb: (request: PcbInspectRequest) =>
    invoke<PcbInspection>("inspect_pcb_job", { request }),
  generatePcb: (request: PcbJobRequest) =>
    invoke<GeneratedPcbJob>("generate_pcb_job", { request }),
  save: (job: GeneratedJob) =>
    invoke<GeneratedGcodeSaveOutcome | undefined>("save_generated_gcode", {
      request: { sourceName: job.sourceName, source: job.source },
    }),
};
