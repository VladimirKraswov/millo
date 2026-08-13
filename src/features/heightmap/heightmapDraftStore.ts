import type { HeightmapPlanRequest } from "../../shared/heightmap";
import { defaultHeightmapRequest } from "./heightmapDefaults";
import type { HeightmapDensity } from "./heightmapModel";

export interface HeightmapDraft {
  readonly schemaVersion: 2;
  readonly request: HeightmapPlanRequest;
  readonly density: HeightmapDensity;
  readonly marginMm: number;
  readonly surfaceSearchMm: number;
  readonly zeroPlateThicknessMm: number;
}

const storageKey = (profileId?: string): string =>
  `millo.heightmap-draft.v2.${encodeURIComponent(profileId ?? "unbound")}`;

const finite = (value: unknown): value is number =>
  typeof value === "number" && Number.isFinite(value);

const browserStorage = (): Storage | undefined =>
  typeof window === "undefined" ? undefined : window.localStorage;

const isRequest = (value: unknown): value is HeightmapPlanRequest => {
  if (!value || typeof value !== "object") return false;
  const request = value as Partial<HeightmapPlanRequest>;
  return [
    request.originXMm,
    request.originYMm,
    request.widthMm,
    request.heightMm,
    request.columns,
    request.rows,
    request.clearanceZMm,
    request.maxProbeDepthMm,
    request.probeFeedMmPerMin,
    request.travelFeedMmPerMin,
    request.retractFeedMmPerMin,
    request.contactOffsetMm,
  ].every(finite) && (request.contactMode === "directSurface" || request.contactMode === "fixedPlate");
};

export const loadHeightmapDraft = (
  profileId?: string,
  storage?: Pick<Storage, "getItem">,
): HeightmapDraft | undefined => {
  try {
    const target = storage ?? browserStorage();
    if (!target) return undefined;
    const raw = target.getItem(storageKey(profileId));
    if (!raw) return undefined;
    const draft = JSON.parse(raw) as Partial<HeightmapDraft>;
    if (
      draft.schemaVersion !== 2 ||
      !isRequest(draft.request) ||
      !["sparse", "normal", "precise", "custom"].includes(draft.density ?? "") ||
      !finite(draft.marginMm) ||
      !finite(draft.surfaceSearchMm) ||
      !finite(draft.zeroPlateThicknessMm)
    ) return undefined;
    return {
      schemaVersion: 2,
      request: draft.request,
      density: draft.density,
      marginMm: draft.marginMm,
      surfaceSearchMm: draft.surfaceSearchMm,
      zeroPlateThicknessMm: draft.zeroPlateThicknessMm,
    } as HeightmapDraft;
  } catch {
    return undefined;
  }
};

export const saveHeightmapDraft = (
  profileId: string | undefined,
  draft: HeightmapDraft,
  storage?: Pick<Storage, "setItem">,
): void => {
  try {
    const target = storage ?? browserStorage();
    if (!target) return;
    target.setItem(storageKey(profileId), JSON.stringify(draft));
  } catch {
    // A storage quota or privacy mode must not block probing.
  }
};

export const initialHeightmapDraft = (
  profileId?: string,
): HeightmapDraft =>
  loadHeightmapDraft(profileId) ?? {
    schemaVersion: 2,
    request: defaultHeightmapRequest(),
    density: "normal",
    marginMm: 1,
    surfaceSearchMm: 10,
    zeroPlateThicknessMm: 0,
  };
