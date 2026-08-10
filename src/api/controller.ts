import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

import type { ControllerSnapshot } from "../shared/machine";

export const isDesktopRuntime = (): boolean => "__TAURI_INTERNALS__" in window;

export const getControllerSnapshot = (): Promise<ControllerSnapshot> =>
  invoke<ControllerSnapshot>("controller_snapshot");

export const connectMock = (): Promise<ControllerSnapshot> =>
  invoke<ControllerSnapshot>("connect_mock");

export const refreshStatus = (): Promise<ControllerSnapshot> =>
  invoke<ControllerSnapshot>("refresh_status");

export const disconnect = (): Promise<ControllerSnapshot> =>
  invoke<ControllerSnapshot>("disconnect");

export const onMachineState = (
  handler: (snapshot: ControllerSnapshot) => void,
): Promise<UnlistenFn> =>
  listen<ControllerSnapshot>("machine-state", (event) => handler(event.payload));
