import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

import type {
  ContinuousJogReceipt,
  ContinuousJogRequest,
  ControllerSnapshot,
  HardwareInspection,
  HomingRequest,
  HomingStartOutcome,
  JogPadStepOutcome,
  JogPadStepRequest,
  MachineOutputOutcome,
  MachineOutputRequest,
  OperatorConfirmation,
  OverrideAdjustment,
  RapidOverrideTarget,
  ReturnToWorkOriginOutcome,
  ReturnToWorkOriginRequest,
  ReturnToWorkZeroOutcome,
  ReturnToWorkZeroRequest,
  ResetChallenge,
  StepJogReceipt,
  StepJogRequest,
  TestJogPreparation,
  TransportDescriptor,
  WorkCoordinateSelectionOutcome,
  WorkCoordinateSystem,
  WorkZeroOutcome,
  WorkZeroRequest,
  ZProbeOutcome,
  ZProbeRequest,
} from "../shared/machine";
import type {
  ConnectOutcome,
  ControllerSettingEditRequest,
  ControllerSettingsState,
} from "../shared/settings";
import type {
  HeightmapOperationSnapshot,
  HeightmapResumeRequest,
  HeightmapStartRequest,
  SurfaceSession,
} from "../shared/heightmap";

export const isDesktopRuntime = (): boolean => "__TAURI_INTERNALS__" in window;

export const getControllerSnapshot = (): Promise<ControllerSnapshot> =>
  invoke<ControllerSnapshot>("controller_snapshot");

export const listTransports = (): Promise<TransportDescriptor[]> =>
  invoke<TransportDescriptor[]>("list_transports");

export const getActiveTransport = (): Promise<TransportDescriptor> =>
  invoke<TransportDescriptor>("active_transport");

export const connectTransport = (
  transportId: string,
  baudRate: number,
): Promise<ConnectOutcome> =>
  invoke<ConnectOutcome>("connect_transport", { transportId, baudRate });

export const getControllerSettings = (): Promise<ControllerSettingsState> =>
  invoke<ControllerSettingsState>("controller_settings");

export const updateControllerSetting = (
  request: ControllerSettingEditRequest,
): Promise<ControllerSettingsState> =>
  invoke<ControllerSettingsState>("update_controller_setting", { request });

export const rollbackControllerSetting = (
  key: string,
  expectedRevision: number,
): Promise<ControllerSettingsState> =>
  invoke<ControllerSettingsState>("rollback_controller_setting", {
    key,
    expectedRevision,
  });

export const refreshStatus = (): Promise<ControllerSnapshot> =>
  invoke<ControllerSnapshot>("refresh_status");

export const inspectDevice = (): Promise<HardwareInspection> =>
  invoke<HardwareInspection>("inspect_device");

export const disconnect = (): Promise<ControllerSnapshot> =>
  invoke<ControllerSnapshot>("disconnect");

export const acknowledgeReset = (): Promise<ControllerSnapshot> =>
  invoke<ControllerSnapshot>("acknowledge_reset");

export const unlockAlarm = (): Promise<ControllerSnapshot> =>
  invoke<ControllerSnapshot>("unlock_alarm");

export const feedHold = (): Promise<ControllerSnapshot> =>
  invoke<ControllerSnapshot>("feed_hold");

export const adjustFeedOverride = (
  adjustment: OverrideAdjustment,
): Promise<ControllerSnapshot> =>
  invoke<ControllerSnapshot>("adjust_feed_override", { adjustment });

export const setRapidOverride = (
  target: RapidOverrideTarget,
): Promise<ControllerSnapshot> =>
  invoke<ControllerSnapshot>("set_rapid_override", { target });

export const adjustSpindleOverride = (
  adjustment: OverrideAdjustment,
): Promise<ControllerSnapshot> =>
  invoke<ControllerSnapshot>("adjust_spindle_override", { adjustment });

export const requestSoftReset = (): Promise<ResetChallenge> =>
  invoke<ResetChallenge>("request_soft_reset");

export const confirmSoftReset = (
  challengeId: number,
): Promise<ControllerSnapshot> =>
  invoke<ControllerSnapshot>("confirm_soft_reset", { challengeId });

export const prepareTestJog = (
  confirmation: OperatorConfirmation,
): Promise<TestJogPreparation> =>
  invoke<TestJogPreparation>("prepare_test_jog", { confirmation });

export const stepJog = (request: StepJogRequest): Promise<StepJogReceipt> =>
  invoke<StepJogReceipt>("step_jog", { request });

export const jogPadStep = (
  request: JogPadStepRequest,
): Promise<JogPadStepOutcome> =>
  invoke<JogPadStepOutcome>("jog_pad_step", { request });

export const startHoming = (request: HomingRequest): Promise<HomingStartOutcome> =>
  invoke<HomingStartOutcome>("start_homing", { request });

export const startContinuousJog = (
  request: ContinuousJogRequest,
): Promise<ContinuousJogReceipt> =>
  invoke<ContinuousJogReceipt>("start_continuous_jog", { request });

export const selectWorkCoordinateSystem = (
  coordinateSystem: WorkCoordinateSystem,
): Promise<WorkCoordinateSelectionOutcome> =>
  invoke<WorkCoordinateSelectionOutcome>("select_work_coordinate_system", {
    coordinateSystem,
  });

export const setMachineOutput = (
  request: MachineOutputRequest,
): Promise<MachineOutputOutcome> =>
  invoke<MachineOutputOutcome>("set_machine_output", { request });

export const setWorkZero = (
  request: WorkZeroRequest,
): Promise<WorkZeroOutcome> =>
  invoke<WorkZeroOutcome>("set_work_zero", { request });

export const returnToWorkZero = (
  request: ReturnToWorkZeroRequest,
): Promise<ReturnToWorkZeroOutcome> =>
  invoke<ReturnToWorkZeroOutcome>("return_to_work_zero", { request });

export const returnToWorkOrigin = (
  request: ReturnToWorkOriginRequest,
): Promise<ReturnToWorkOriginOutcome> =>
  invoke<ReturnToWorkOriginOutcome>("return_to_work_origin", { request });

export const runZProbe = (request: ZProbeRequest): Promise<ZProbeOutcome> =>
  invoke<ZProbeOutcome>("probe_z", { request });

export const getSurfaceSession = (): Promise<SurfaceSession> =>
  invoke<SurfaceSession>("surface_session");

export const getHeightmapSnapshot = (): Promise<HeightmapOperationSnapshot> =>
  invoke<HeightmapOperationSnapshot>("heightmap_snapshot");

export const startHeightmap = (
  request: HeightmapStartRequest,
  machineProfileId: string,
): Promise<HeightmapOperationSnapshot> =>
  invoke<HeightmapOperationSnapshot>("start_heightmap", { request, machineProfileId });

export const resumeHeightmapDraft = (
  request: HeightmapResumeRequest,
  machineProfileId: string,
): Promise<HeightmapOperationSnapshot> =>
  invoke<HeightmapOperationSnapshot>("resume_heightmap_draft", { request, machineProfileId });

export const pauseHeightmap = (): Promise<HeightmapOperationSnapshot> =>
  invoke<HeightmapOperationSnapshot>("pause_heightmap");

export const resumeHeightmap = (): Promise<HeightmapOperationSnapshot> =>
  invoke<HeightmapOperationSnapshot>("resume_heightmap");

export const cancelHeightmap = (): Promise<HeightmapOperationSnapshot> =>
  invoke<HeightmapOperationSnapshot>("cancel_heightmap");

export const setHeightmapApplication = (
  enabled: boolean,
  setupConfirmed: boolean,
): Promise<SurfaceSession> =>
  invoke<SurfaceSession>("set_heightmap_application", { enabled, setupConfirmed });

export const clearSurfaceSession = (): Promise<SurfaceSession> =>
  invoke<SurfaceSession>("clear_surface_session");

export const discardHeightmapDraft = (): Promise<SurfaceSession> =>
  invoke<SurfaceSession>("discard_heightmap_draft");

export const onHeightmapState = (
  handler: (snapshot: HeightmapOperationSnapshot) => void,
): Promise<UnlistenFn> =>
  listen<HeightmapOperationSnapshot>("heightmap-state", (event) => handler(event.payload));

export const onSurfaceSession = (
  handler: (session: SurfaceSession) => void,
): Promise<UnlistenFn> =>
  listen<SurfaceSession>("surface-session", (event) => handler(event.payload));

export const cancelJog = (): Promise<ControllerSnapshot> =>
  invoke<ControllerSnapshot>("cancel_jog");

export const onMachineState = (
  handler: (snapshot: ControllerSnapshot) => void,
): Promise<UnlistenFn> =>
  listen<ControllerSnapshot>("machine-state", (event) => handler(event.payload));
