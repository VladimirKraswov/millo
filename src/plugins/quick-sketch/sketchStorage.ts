import type { SketchJobRequest } from "../../shared/sketch";
import { emptySketch } from "./sketchModel";
import { resolveSketch } from "./sketchConstraints";

const key = "millo.quick-sketch.v1";
// Stored drafts are untrusted input too. CAM performs the authoritative geometric validation.
export function decodeSketch(text: string): SketchJobRequest {
  if (text.length > 512_000) throw new Error("Чертёж больше 512 КБ");
  const data: unknown = JSON.parse(text);
  const object = (v: unknown): v is Record<string, unknown> =>
    Boolean(v && typeof v === "object" && !Array.isArray(v));
  const numbers = (v: Record<string, unknown>, keys: string[]) =>
    keys.every((k) => typeof v[k] === "number" && Number.isFinite(v[k]));
  const bounded = (v: unknown, min: number, max: number) =>
    typeof v === "number" && Number.isFinite(v) && v >= min && v <= max;
  const fail = () => {
    throw new Error("Некорректный формат чертежа Millo (v1/v2)");
  };
  if (
    !object(data) ||
    ![1, 2].includes(Number(data.version)) ||
    typeof data.version !== "number" ||
    !object(data.document)
  )
    return fail();
  const doc = data.document;
  if (
    typeof doc.sourceName !== "string" ||
    !object(doc.stock) ||
    !numbers(doc.stock, [
      "widthMm",
      "heightMm",
      "thicknessMm",
      "safeZMm",
      "breakthroughMm",
    ]) ||
    !["manual", "controller"].includes(String(doc.stock.spindleMode)) ||
    !Array.isArray(doc.shapes) ||
    doc.shapes.length > 200
  )
    return fail();
  if (
    doc.sourceName.length > 100 ||
    !bounded(doc.stock.widthMm, 1, 10_000) ||
    !bounded(doc.stock.heightMm, 1, 10_000) ||
    !bounded(doc.stock.thicknessMm, 0.05, 100) ||
    !bounded(doc.stock.safeZMm, 0.5, 100) ||
    !bounded(doc.stock.breakthroughMm, 0, 1)
  )
    return fail();
  const ids = new Set<string>();
  for (const s of doc.shapes) {
    if (
      !object(s) ||
      typeof s.id !== "string" ||
      typeof s.name !== "string" ||
      !numbers(s, ["xMm", "yMm", "rotationDegrees"]) ||
      !object(s.geometry) ||
      !object(s.operation)
    )
      return fail();
    const g = s.geometry,
      op = s.operation;
    if (
      !s.id ||
      ids.has(s.id) ||
      s.id.length > 100 ||
      s.name.length > 120 ||
      !bounded(s.xMm, -10_000, 10_000) ||
      !bounded(s.yMm, -10_000, 10_000) ||
      !bounded(s.rotationDegrees, -360, 360)
    )
      return fail();
    ids.add(s.id);
    if (s.locked !== undefined && typeof s.locked !== "boolean") return fail();
    if (s.constraints !== undefined) {
      if (
        !object(s.constraints) ||
        Object.keys(s.constraints).some((k) => k !== "x" && k !== "y")
      )
        return fail();
      const validAnchor = (v: unknown) =>
        ["min", "center", "max"].includes(String(v)) ||
        (Number.isInteger(v) && bounded(v, 0, 255));
      for (const binding of Object.values(s.constraints)) {
        if (
          !object(binding) ||
          !validAnchor(binding.referenceAnchor) ||
          !validAnchor(binding.ownAnchor) ||
          !bounded(binding.offsetMm, -10_000, 10_000) ||
          (binding.referenceId !== undefined &&
            (typeof binding.referenceId !== "string" ||
              !binding.referenceId ||
              binding.referenceId.length > 100))
        )
          return fail();
      }
    }
    if (g.kind === "rectangle") {
      if (!numbers(g, ["width", "height", "radius"])) return fail();
    } else if (g.kind === "circle") {
      if (!numbers(g, ["diameter"])) return fail();
    } else if (g.kind === "polygon") {
      if (
        !Array.isArray(g.points) ||
        g.points.length < 3 ||
        g.points.length > 256 ||
        !g.points.every((p) => object(p) && numbers(p, ["x", "y"]))
      )
        return fail();
    } else return fail();
    if (g.kind === "circle" && !bounded(g.diameter, 0.1, 10_000)) return fail();
    if (
      g.kind === "rectangle" &&
      (!bounded(g.width, 0.1, 10_000) ||
        !bounded(g.height, 0.1, 10_000) ||
        !bounded(g.radius, 0, Math.min(Number(g.width), Number(g.height)) / 2))
    )
      return fail();
    if (
      g.kind === "polygon" &&
      (g.points as { x: number; y: number }[]).some(
        (p) => !bounded(p.x, -10_000, 10_000) || !bounded(p.y, -10_000, 10_000),
      )
    )
      return fail();
    if (
      !["pocket", "inside", "outside", "engrave", "drill"].includes(
        String(op.kind),
      ) ||
      typeof op.toolId !== "string" ||
      typeof op.through !== "boolean" ||
      !numbers(op, [
        "depthMm",
        "stepdownMm",
        "stepoverPercent",
        "feedMmPerMin",
        "plungeMmPerMin",
        "spindleRpm",
      ]) ||
      !object(op.tabs) ||
      !numbers(op.tabs, ["count", "widthMm", "heightMm"])
    )
      return fail();
    if (
      !bounded(op.depthMm, 0.01, 101) ||
      !bounded(op.stepdownMm, 0.01, 10) ||
      !bounded(op.stepoverPercent, 5, 50) ||
      !bounded(op.feedMmPerMin, 1, 30_000) ||
      !bounded(op.plungeMmPerMin, 1, 10_000) ||
      !bounded(op.spindleRpm, 1000, 100_000) ||
      !Number.isInteger(op.spindleRpm) ||
      !Number.isInteger(op.tabs.count) ||
      !bounded(op.tabs.count, 0, 16) ||
      !bounded(op.tabs.widthMm, 0.5, 50) ||
      !bounded(op.tabs.heightMm, 0.05, 100)
    )
      return fail();
  }
  return resolveSketch(doc as unknown as SketchJobRequest);
}
export function loadDraft(): SketchJobRequest {
  try {
    const value = localStorage.getItem(key);
    return value ? decodeSketch(value) : emptySketch();
  } catch {
    return emptySketch();
  }
}
export function saveDraft(doc: SketchJobRequest): boolean {
  try {
    localStorage.setItem(key, JSON.stringify({ version: 2, document: doc }));
    return true;
  } catch {
    return false;
  }
}
