import { describe, expect, it, vi } from "vitest";

import type {
  GeneratedImageJob,
  GeneratedPcbJob,
  GeneratedSurfacingJob,
  ImageJobRequest,
  PcbInspectRequest,
  PcbJobRequest,
  SurfacingJobRequest,
} from "../../shared/jobs";
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
const generatedSurfacing = {
  sourceName: "surface.nc",
  source: "G21\nM5\nM30\n",
  program: { lines: [], warnings: [], features: {}, summary: {}, toolpath: [] },
  summary: {},
} as unknown as GeneratedSurfacingJob;
const generatedPcb = {
  sourceName: "board.nc",
  source: "G21\nM5\nM30\n",
  program: { lines: [], warnings: [], features: {}, summary: {}, toolpath: [] },
  inspection: { bounds: {}, paths: [], drillHits: [], drillSlots: [], drillGroups: [], files: [], warnings: [] },
  summary: {},
} as unknown as GeneratedPcbJob;

describe("JobCreationService", () => {
  it("publishes only immutable jobs returned by the core gateway", async () => {
    const store = new GeneratedJobStore();
    const gateway: ImageJobGateway = {
      generate: vi.fn(async () => generated),
      generateSurfacing: vi.fn(),
      inspectPcb: vi.fn(),
      generatePcb: vi.fn(),
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
      generateSurfacing: vi.fn(),
      inspectPcb: vi.fn(),
      generatePcb: vi.fn(),
      save: vi.fn(),
    };
    const service = new JobCreationService(gateway, new GeneratedJobStore());

    expect(() => service.open(generated)).toThrow("not issued");
    expect(() => service.save(generated)).toThrow("not issued");
    expect(gateway.save).not.toHaveBeenCalled();
  });

  it("issues surfacing jobs through the same immutable open/save boundary", async () => {
    const store = new GeneratedJobStore();
    const request = {} as SurfacingJobRequest;
    const gateway: ImageJobGateway = {
      generate: vi.fn(),
      generateSurfacing: vi.fn(async () => generatedSurfacing),
      inspectPcb: vi.fn(),
      generatePcb: vi.fn(),
      save: vi.fn(async () => undefined),
    };
    const service = new JobCreationService(gateway, store);

    const job = await service.generateSurfacing(request);
    service.open(job);
    await service.save(job);

    expect(gateway.generateSurfacing).toHaveBeenCalledWith(request);
    expect(gateway.save).toHaveBeenCalledWith(job);
    expect(store.current()?.job).toBe(job);
    expect(Object.isFrozen(job)).toBe(true);
  });

  it("keeps PCB inspection read-only and issues only generated PCB jobs", async () => {
    const inspection = { paths: [], drillHits: [] } as unknown as Awaited<ReturnType<ImageJobGateway["inspectPcb"]>>;
    const gateway: ImageJobGateway = {
      generate: vi.fn(),
      generateSurfacing: vi.fn(),
      inspectPcb: vi.fn(async () => inspection),
      generatePcb: vi.fn(async () => generatedPcb),
      save: vi.fn(),
    };
    const service = new JobCreationService(gateway, new GeneratedJobStore());

    const inspected = await service.inspectPcb({} as PcbInspectRequest);
    const job = await service.generatePcb({} as PcbJobRequest);

    expect(Object.isFrozen(inspected)).toBe(true);
    expect(Object.isFrozen(job)).toBe(true);
    expect(() => service.open(inspected as unknown as GeneratedPcbJob)).toThrow("not issued");
    expect(() => service.open(job)).not.toThrow();
  });
});
