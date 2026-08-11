import type { ReactNode } from "react";

import type { MachineCommandGateway } from "../machine/MachineCommandGateway";
import type { WorkCoordinateGateway } from "../machine/WorkCoordinateGateway";
import type {
  ControllerSnapshot,
  HardwareInspection,
} from "../../shared/machine";
import { ExtensionRegistry } from "./ExtensionRegistry";

export const uiSlots = {
  controlMachine: "control.machine",
  controlCoordinates: "control.coordinates",
} as const;

export type UiSlotId = (typeof uiSlots)[keyof typeof uiSlots];

export interface UiHostContext {
  readonly snapshot: ControllerSnapshot;
  readonly desktopRuntime: boolean;
  readonly controlsDisabled: boolean;
  readonly machineCommands: MachineCommandGateway;
  readonly workCoordinates: WorkCoordinateGateway;
  readonly updateSnapshot: (snapshot: ControllerSnapshot) => void;
  readonly updateInspection: (inspection?: HardwareInspection) => void;
  readonly reportError: (error?: string) => void;
}

export type UiExtension = (context: UiHostContext) => ReactNode;
export type UiExtensionRegistry = ExtensionRegistry<UiSlotId, UiExtension>;

export const createUiExtensionRegistry = (): UiExtensionRegistry =>
  new ExtensionRegistry<UiSlotId, UiExtension>();
