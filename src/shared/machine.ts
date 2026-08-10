export type ConnectionState =
  | "disconnected"
  | "connecting"
  | "connected"
  | "faulted";

export type MachineMode =
  | "unknown"
  | "idle"
  | "run"
  | "hold"
  | "jog"
  | "alarm"
  | "door"
  | "check"
  | "home"
  | "sleep";

export interface Position {
  x: number;
  y: number;
  z: number;
  a?: number;
}

export interface MachineState {
  mode: MachineMode;
  reportedMode: string;
  substate?: number;
  machinePosition?: Position;
  workPosition?: Position;
  workCoordinateOffset?: Position;
  feedRate: number;
  spindleSpeed: number;
}

export interface ControllerSnapshot {
  connection: ConnectionState;
  machine: MachineState;
  lastError?: string;
}

export const emptySnapshot: ControllerSnapshot = {
  connection: "disconnected",
  machine: {
    mode: "unknown",
    reportedMode: "Unknown",
    feedRate: 0,
    spindleSpeed: 0,
  },
};
