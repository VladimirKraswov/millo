import { invoke } from "@tauri-apps/api/core";

import type {
  GeneratedGcodeSaveOutcome,
  GeneratedImageJob,
  ImageJobRequest,
} from "../../shared/jobs";
import type { ImageJobGateway } from "./ImageJobGateway";

export const tauriImageJobGateway: ImageJobGateway = {
  generate: (request: ImageJobRequest) =>
    invoke<GeneratedImageJob>("generate_image_job", { request }),
  save: (job: GeneratedImageJob) =>
    invoke<GeneratedGcodeSaveOutcome | undefined>("save_generated_gcode", {
      request: { sourceName: job.sourceName, source: job.source },
    }),
};
