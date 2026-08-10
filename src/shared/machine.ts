export type ConnectionState =
  | "disconnected"
  | "connecting"
  | "connected"
  | "recovering"
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

export interface ResetNotice {
  banner: string;
  version?: string;
  sequence: number;
}

export interface AlarmState {
  code?: number;
  message: string;
}

export interface ControllerSnapshot {
  connection: ConnectionState;
  machine: MachineState;
  resetNotice?: ResetNotice;
  alarm?: AlarmState;
  consecutiveFailures: number;
  reconnectCount: number;
  pollSequence: number;
  resetCount: number;
  pollIntervalMs: number;
  statusTimeoutMs: number;
  failureThreshold: number;
  lastError?: string;
}

export type TransportKind = "mock" | "serial";

export interface TransportDescriptor {
  id: string;
  kind: TransportKind;
  label: string;
  detail?: string;
  portName?: string;
  likelyGrbl: boolean;
  matchReason?: string;
}

export type CommandCompletion = "ok" | "error" | "alarm" | "reset";

export interface CommandResponse {
  command: string;
  completion: CommandCompletion;
  lines: string[];
  code?: number;
}

export interface DeviceInspection {
  firmwareVersion?: string;
  firmwareBuildInfo?: string;
  firmwareOptions?: string;
  settings: Record<string, string>;
  modalState: string[];
  parameters: Record<string, string>;
  responses: CommandResponse[];
}

export const emptySnapshot: ControllerSnapshot = {
  connection: "disconnected",
  machine: {
    mode: "unknown",
    reportedMode: "Unknown",
    feedRate: 0,
    spindleSpeed: 0,
  },
  consecutiveFailures: 0,
  reconnectCount: 0,
  pollSequence: 0,
  resetCount: 0,
  pollIntervalMs: 0,
  statusTimeoutMs: 0,
  failureThreshold: 0,
};
