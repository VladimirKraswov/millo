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

export interface ControllerBufferState {
  plannerAvailable: number;
  rxAvailable: number;
}

export interface ControllerOverrides {
  feedPercent: number;
  rapidPercent: number;
  spindlePercent: number;
}

export type OverrideAdjustment =
  | "reset"
  | "increaseTen"
  | "decreaseTen"
  | "increaseOne"
  | "decreaseOne";

export type RapidOverrideTarget = "full" | "half" | "quarter";

export interface ControllerPins {
  raw: string;
  xLimit: boolean;
  yLimit: boolean;
  zLimit: boolean;
  aLimit: boolean;
  bLimit: boolean;
  cLimit: boolean;
  probe: boolean;
  door: boolean;
  hold: boolean;
  softReset: boolean;
  cycleStart: boolean;
}

export interface ControllerAccessories {
  raw: string;
  spindleClockwise: boolean;
  spindleCounterclockwise: boolean;
  floodCoolant: boolean;
  mistCoolant: boolean;
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
  bufferState?: ControllerBufferState;
  overrides?: ControllerOverrides;
  pins?: ControllerPins;
  accessories?: ControllerAccessories;
  lineNumber?: number;
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
  vendorId?: number;
  productId?: number;
  manufacturer?: string;
  product?: string;
  serialNumber?: string;
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
  controllerCapabilities?: ControllerCapabilities;
  settings: Record<string, string>;
  modalState: string[];
  parameters: Record<string, string>;
  responses: CommandResponse[];
}

export interface ControllerCapabilities {
  optionFlags: string;
  plannerBufferBlocks?: number;
  rxBufferBytes?: number;
}

export type SpindleControl = "manual" | "controller";

export interface MachineTravel {
  x: number;
  y: number;
  z: number;
}

export interface HardwareProfile {
  name: string;
  axes: string[];
  travelMm?: MachineTravel;
  spindleControl: SpindleControl;
  homingInstalled: boolean;
  limitSwitchesInstalled: boolean;
  probeInstalled: boolean;
  emergencyStopInstalled: boolean;
}

export type ReadinessLevel = "pass" | "caution" | "blocker";

export interface ReadinessCheck {
  id: string;
  level: ReadinessLevel;
  title: string;
  detail: string;
  evidence?: string;
}

export interface ReadinessReport {
  profile: HardwareProfile;
  testJogReady: boolean;
  probeReady: boolean;
  blockerCount: number;
  cautionCount: number;
  checks: ReadinessCheck[];
}

export interface HardwareInspection {
  device: DeviceInspection;
  readiness: ReadinessReport;
}

export interface OperatorConfirmation {
  spindleOff: boolean;
  toolClear: boolean;
  powerControlReachable: boolean;
}

export interface ResetChallenge {
  id: number;
  expiresInMs: number;
}

export interface TestJogAuthorization {
  id: number;
  expiresInMs: number;
}

export interface TestJogPreparation {
  inspection: HardwareInspection;
  authorization?: TestJogAuthorization;
}

export type JogAxis = "x" | "y" | "z";

export interface StepJogRequest {
  authorizationId: number;
  axis: JogAxis;
  distanceMm: number;
  feedMmPerMin: number;
}

export interface StepJogReceipt {
  command: string;
  axis: JogAxis;
  distanceMm: number;
  feedMmPerMin: number;
}

export interface JogPadStepRequest {
  confirmation: OperatorConfirmation;
  axis: JogAxis;
  distanceMm: number;
}

export interface JogPadStepOutcome {
  inspection: HardwareInspection;
  receipt?: StepJogReceipt;
}

export type WorkAxis = "x" | "y" | "z";
export type WorkCoordinateSystem = "g54" | "g55" | "g56" | "g57" | "g58" | "g59";

export interface WorkZeroRequest {
  axis: WorkAxis;
  positionConfirmed: boolean;
}

export interface WorkZeroOutcome {
  axis: WorkAxis;
  coordinateSystem: WorkCoordinateSystem;
  command: string;
  parameterValue: string;
  workPosition: number;
  snapshot: ControllerSnapshot;
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
