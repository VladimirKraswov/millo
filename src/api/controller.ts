import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

import type {
  ControllerSnapshot,
  HardwareInspection,
  JogPadStepOutcome,
  JogPadStepRequest,
  OperatorConfirmation,
  OverrideAdjustment,
  RapidOverrideTarget,
  ReturnToWorkZeroOutcome,
  ReturnToWorkZeroRequest,
  ResetChallenge,
  StepJogReceipt,
  StepJogRequest,
  TestJogPreparation,
  TransportDescriptor,
  WorkZeroOutcome,
  WorkZeroRequest,
} from "../shared/machine";
import type {
  ConnectOutcome,
  ControllerSettingEditRequest,
  ControllerSettingsState,
} from "../shared/settings";

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

export const setWorkZero = (
  request: WorkZeroRequest,
): Promise<WorkZeroOutcome> =>
  invoke<WorkZeroOutcome>("set_work_zero", { request });

export const returnToWorkZero = (
  request: ReturnToWorkZeroRequest,
): Promise<ReturnToWorkZeroOutcome> =>
  invoke<ReturnToWorkZeroOutcome>("return_to_work_zero", { request });

export const cancelJog = (): Promise<ControllerSnapshot> =>
  invoke<ControllerSnapshot>("cancel_jog");

export const triggerMockReset = (): Promise<void> =>
  invoke<void>("mock_trigger_reset");

export const triggerMockRun = (): Promise<void> =>
  invoke<void>("mock_start_run");

export const triggerMockAlarm = (code = 3): Promise<void> =>
  invoke<void>("mock_trigger_alarm", { code });

export const clearMockAlarm = (): Promise<void> => invoke<void>("mock_clear_alarm");

export const triggerMockTimeout = (): Promise<void> =>
  invoke<void>("mock_trigger_timeout");

export const triggerMockDisconnect = (): Promise<void> =>
  invoke<void>("mock_trigger_disconnect");

export const onMachineState = (
  handler: (snapshot: ControllerSnapshot) => void,
): Promise<UnlistenFn> =>
  listen<ControllerSnapshot>("machine-state", (event) => handler(event.payload));
