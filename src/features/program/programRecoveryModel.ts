import type {
  ProgramRecoveryCandidate,
  ProgramRecoveryPreparationRequest,
} from "../../shared/recovery";

export type RecoveryConfirmationKey = Exclude<
  keyof ProgramRecoveryPreparationRequest,
  "recoveryId" | "safeZMm" | "continuity"
>;

export const recoverySafeZDefault = (
  candidate: ProgramRecoveryCandidate,
): number => Math.ceil(((candidate.minimumSafeZMm ?? 0) + 2) * 10) / 10;

export const emptyRecoveryPreparation = (
  candidate: ProgramRecoveryCandidate,
): ProgramRecoveryPreparationRequest => ({
  recoveryId: candidate.id,
  safeZMm: recoverySafeZDefault(candidate),
  continuity: "motionPowerLostOrUnknown",
  machineReferenceRestored: false,
  workZeroRestored: false,
  motionPowerRestored: false,
  restartPointInspected: false,
  pathClear: false,
  powerControlReachable: false,
});

export const canPrepareRecovery = (
  candidate: ProgramRecoveryCandidate,
  request: ProgramRecoveryPreparationRequest,
  busy: boolean,
): boolean =>
  candidate.ready &&
  (request.continuity === "motionPowerLostOrUnknown"
    ? candidate.fullRestartAvailable
    : candidate.checkpointRestartAvailable) &&
  !busy &&
  Number.isFinite(request.safeZMm) &&
  request.safeZMm >= (candidate.minimumSafeZMm ?? Number.POSITIVE_INFINITY) &&
  request.machineReferenceRestored &&
  request.workZeroRestored &&
  request.motionPowerRestored &&
  request.restartPointInspected &&
  request.pathClear &&
  request.powerControlReachable;
