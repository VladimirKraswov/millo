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
import type { GeneratedSketchJob, SketchJobRequest } from "../../shared/sketch";

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
  inspection: { bounds: {}, paths: [], drillHits: [], drillSlots: [], drillGroups: [], files: [], copperAnalysis: { contourCount: 0 }, warnings: [] },
  summary: {
    operations: [
      { kind: "isolation", toolNumber: 1, toolId: "v-bit", toolName: "V-bit", motionCount: 8 },
      { kind: "marking", toolNumber: 1, toolId: "v-bit", toolName: "V-bit", motionCount: 3 },
      { kind: "outline", toolNumber: 2, toolId: "end-mill", toolName: "End mill", motionCount: 5 },
    ],
  },
} as unknown as GeneratedPcbJob;

describe("JobCreationService", () => {
  it("publishes trusted sketch tool assignments but never publishes saved projects", async () => {
    const store = new GeneratedJobStore();
    const result = {
      ...generatedSurfacing,
      summary: {
        operations: [
          { toolNumber: 1, toolId: "small" },
          { toolNumber: 1, toolId: "small" },
          { toolNumber: 2, toolId: "large" },
        ],
        paths: [], tabPaths: [], warnings: [], toolChangeCount: 1,
      },
    } as unknown as GeneratedSketchJob;
    const gateway: ImageJobGateway = {
      generate: vi.fn(), generateSurfacing: vi.fn(),
      inspectPcb: vi.fn(), generatePcb: vi.fn(),
      generateSketch: vi.fn(async () => result),
      saveSketchProject: vi.fn(async () => undefined), save: vi.fn(),
    };
    const service = new JobCreationService(gateway, store);
    const draft = {} as SketchJobRequest;
    await service.saveSketchProject(draft);
    expect(gateway.saveSketchProject).toHaveBeenCalledWith(draft);
    expect(store.current()).toBeUndefined();
    const job = await service.generateSketch(draft);
    expect(job.toolAssignments).toEqual([
      { toolNumber: 1, toolId: "small" }, { toolNumber: 2, toolId: "large" },
    ]);
    expect(Object.isFrozen(job.summary.operations)).toBe(true);
    expect(store.current()).toBeUndefined();
    service.open(job);
    expect(store.current()?.job).toBe(job);
  });
  it("publishes only immutable jobs returned by the core gateway", async () => {
    const store = new GeneratedJobStore();
    const gateway: ImageJobGateway = {
      generate: vi.fn(async () => generated),
      generateSurfacing: vi.fn(),
      inspectPcb: vi.fn(),
      generatePcb: vi.fn(),
      generateSketch: vi.fn(),
      saveSketchProject: vi.fn(),
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
      generateSketch: vi.fn(),
      saveSketchProject: vi.fn(),
      save: vi.fn(),
    };
    const service = new JobCreationService(gateway, new GeneratedJobStore());

    expect(() => service.open(generated)).toThrow("not issued");
    expect(() => service.save(generated)).toThrow("not issued");
    expect(gateway.save).not.toHaveBeenCalled();
  });

  it("issues surfacing jobs through the same immutable open/save boundary", async () => {
    const store = new GeneratedJobStore();
    const request = { toolId: "surfacing-22mm" } as SurfacingJobRequest;
    const gateway: ImageJobGateway = {
      generate: vi.fn(),
      generateSurfacing: vi.fn(async () => generatedSurfacing),
      inspectPcb: vi.fn(),
      generatePcb: vi.fn(),
      generateSketch: vi.fn(),
      saveSketchProject: vi.fn(),
      save: vi.fn(async () => undefined),
    };
    const service = new JobCreationService(gateway, store);

    const job = await service.generateSurfacing(request);
    service.open(job);
    await service.save(job);

    expect(gateway.generateSurfacing).toHaveBeenCalledWith(request);
    expect(gateway.save).toHaveBeenCalledWith(job);
    expect(store.current()?.job).toBe(job);
    expect(job.toolAssignments).toEqual([
      { toolNumber: 1, toolId: "surfacing-22mm" },
    ]);
    expect(Object.isFrozen(job)).toBe(true);
    expect(Object.isFrozen(job.toolAssignments)).toBe(true);
  });

  it("keeps PCB inspection read-only and issues only generated PCB jobs", async () => {
    const inspection = { paths: [], drillHits: [] } as unknown as Awaited<ReturnType<ImageJobGateway["inspectPcb"]>>;
    const gateway: ImageJobGateway = {
      generate: vi.fn(),
      generateSurfacing: vi.fn(),
      inspectPcb: vi.fn(async () => inspection),
      generatePcb: vi.fn(async () => generatedPcb),
      generateSketch: vi.fn(),
      saveSketchProject: vi.fn(),
      save: vi.fn(),
    };
    const service = new JobCreationService(gateway, new GeneratedJobStore());

    const inspected = await service.inspectPcb({} as PcbInspectRequest);
    const job = await service.generatePcb({} as PcbJobRequest);

    expect(Object.isFrozen(inspected)).toBe(true);
    expect(Object.isFrozen(job)).toBe(true);
    expect(job.toolAssignments).toEqual([
      { toolNumber: 1, toolId: "v-bit" },
      { toolNumber: 2, toolId: "end-mill" },
    ]);
    expect(() => service.open(inspected as unknown as GeneratedPcbJob)).toThrow("not issued");
    expect(() => service.open(job)).not.toThrow();
  });
});
