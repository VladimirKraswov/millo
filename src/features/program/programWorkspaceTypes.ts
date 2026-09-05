import type { HeightmapGateway } from "../../platform/machine/HeightmapGateway";
import type { ProgramGateway } from "../../platform/program/ProgramGateway";
import {
  type SenderSnapshot,
  type SenderStateGateway,
} from "../../shared/dryRun";
import type { JobToolAssignment, PublishedJob } from "../../shared/jobs";
import type {
  ControllerSnapshot,
  HardwareInspection,
  Position,
} from "../../shared/machine";
import type { GcodeProgram } from "../../shared/program";
import type {
  ProgramRunIntent,
  RealRunPreflightGateway,
  SafeStartPackage,
} from "../../shared/realRun";
import type { CuttingTool } from "../../shared/tooling";
import { type LoadedProgram } from "./ProgramLoader";
export interface ProgramMachineContext {
  readonly activeCoordinateSystem: string;
  readonly busy: boolean;
  readonly machineBound: boolean;
  readonly machineName: string;
  readonly machineProfileId?: string;
  readonly machineSyncing: boolean;
  readonly onAcknowledgeReset: () => void | Promise<unknown>;
  readonly onConnect: () => void | Promise<unknown>;
  readonly onOpenWorkZero: () => void;
  readonly onReturnToWorkOrigin: (clearanceZMm: number) => Promise<void>;
  readonly onSyncMachine: () => void | Promise<unknown>;
  readonly onUnlock: () => void | Promise<unknown>;
  readonly snapshot: ControllerSnapshot;
  readonly workPosition?: Position;
}

export interface ProgramWorkspaceProps {
  readonly desktopRuntime: boolean;
  readonly gateway: ProgramGateway;
  readonly heightmapGateway?: HeightmapGateway;
  readonly initialProgram?: GcodeProgram;
  readonly initialRunIntent?: ProgramRunIntent;
  readonly initialSender?: SenderSnapshot;
  readonly initialSource?: string;
  readonly initialToolAssignments?: readonly JobToolAssignment[];
  readonly incomingJob?: PublishedJob;
  readonly machineContext?: ProgramMachineContext;
  readonly onInspection?: (inspection: HardwareInspection) => void;
  readonly onProgramChange?: (program?: GcodeProgram) => void;
  readonly onError?: (message: string) => void;
  readonly realRunAvailable?: boolean;
  readonly realRunGateway?: RealRunPreflightGateway;
  readonly realRunTarget?: boolean;
  readonly senderGateway?: SenderStateGateway;
  readonly tools?: readonly CuttingTool[];
}

export interface SafeStartContext {
  readonly original: LoadedProgram;
  readonly package: SafeStartPackage;
}
