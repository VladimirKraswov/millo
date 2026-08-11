import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

import type {
  ControllerSnapshot,
  HardwareInspection,
  OperatorConfirmation,
  ResetChallenge,
  StepJogReceipt,
  StepJogRequest,
  TestJogPreparation,
  TransportDescriptor,
} from "../shared/machine";

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
): Promise<ControllerSnapshot> =>
  invoke<ControllerSnapshot>("connect_transport", { transportId, baudRate });

export const refreshStatus = (): Promise<ControllerSnapshot> =>
  invoke<ControllerSnapshot>("refresh_status");

export const inspectDevice = (): Promise<HardwareInspection> =>
  invoke<HardwareInspection>("inspect_device");

export const disconnect = (): Promise<ControllerSnapshot> =>
  invoke<ControllerSnapshot>("disconnect");

export const acknowledgeReset = (): Promise<ControllerSnapshot> =>
  invoke<ControllerSnapshot>("acknowledge_reset");

export const feedHold = (): Promise<ControllerSnapshot> =>
  invoke<ControllerSnapshot>("feed_hold");

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
