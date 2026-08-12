import { describe, expect, it, vi } from "vitest";

import type { GeneratedImageJob, ImageJobRequest } from "../../shared/jobs";
import type { ImageJobGateway } from "./ImageJobGateway";
import { GeneratedJobStore } from "./GeneratedJobStore";
import { JobCreationService } from "./JobCreationService";

const request = {} as ImageJobRequest;
const generated = {
  sourceName: "mark.nc",
  source: "G21\nM30\n",
  vectorSvg: "<svg/>",
  program: { lines: [], warnings: [], features: {}, summary: {}, toolpath: [] },
  summary: {},
} as unknown as GeneratedImageJob;

describe("JobCreationService", () => {
  it("publishes only immutable jobs returned by the core gateway", async () => {
    const store = new GeneratedJobStore();
    const gateway: ImageJobGateway = {
      generate: vi.fn(async () => generated),
      save: vi.fn(),
    };
    const service = new JobCreationService(gateway, store);
    const job = await service.generateImage(request);

    service.open(job);

    expect(store.current()?.job).toBe(job);
    expect(Object.isFrozen(job)).toBe(true);
    expect(Object.isFrozen(job.program)).toBe(true);
  });

  it("rejects fabricated jobs for open and save", async () => {
    const gateway: ImageJobGateway = {
      generate: vi.fn(async () => generated),
      save: vi.fn(),
    };
    const service = new JobCreationService(gateway, new GeneratedJobStore());

    expect(() => service.open(generated)).toThrow("not issued");
    expect(() => service.save(generated)).toThrow("not issued");
    expect(gateway.save).not.toHaveBeenCalled();
  });
});
