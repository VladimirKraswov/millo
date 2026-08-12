import type {
  ProgramRecoveryCandidate,
  ProgramRecoveryPreparationRequest,
} from "../../shared/recovery";

export type RecoveryConfirmationKey = Exclude<
  keyof ProgramRecoveryPreparationRequest,
  "recoveryId" | "safeZMm"
>;

export const recoverySafeZDefault = (
  candidate: ProgramRecoveryCandidate,
): number => Math.ceil(((candidate.minimumSafeZMm ?? 0) + 2) * 10) / 10;

export const emptyRecoveryPreparation = (
  candidate: ProgramRecoveryCandidate,
): ProgramRecoveryPreparationRequest => ({
  recoveryId: candidate.id,
  safeZMm: recoverySafeZDefault(candidate),
  machineReferenceRestored: false,
  workZeroRestored: false,
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
  !busy &&
  Number.isFinite(request.safeZMm) &&
  request.safeZMm >= (candidate.minimumSafeZMm ?? Number.POSITIVE_INFINITY) &&
  request.machineReferenceRestored &&
  request.workZeroRestored &&
  request.restartPointInspected &&
  request.pathClear &&
  request.powerControlReachable;
