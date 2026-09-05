import { ChevronDown } from "lucide-react";

import {
  uiSlots,
  type UiExtensionRegistry,
} from "../platform/extensions/UiExtensionRegistry";
import { UiExtensionSlot } from "../platform/extensions/UiExtensionSlot";
import { MachineReferencePanel } from "../features/machine-control/MachineReferencePanel";
import { MachineSetupPanel } from "../features/machine-control/MachineSetupPanel";
import type { MachineCommandGateway } from "../platform/machine/MachineCommandGateway";
import type { WorkCoordinateGateway } from "../platform/machine/WorkCoordinateGateway";
import type {
  ControllerSnapshot,
  HardwareInspection,
  SpindleControl,
  RotaryAxisProfile,
  WorkCoordinateSystem,
} from "../shared/machine";

interface SafetyControlsProps {
  snapshot: ControllerSnapshot;
  desktopRuntime: boolean;
  extensionRegistry: UiExtensionRegistry;
  machineGateway: MachineCommandGateway;
  workCoordinateGateway: WorkCoordinateGateway;
  machineBound: boolean;
  onSnapshot: (snapshot: ControllerSnapshot) => void;
  onInspection: (inspection?: HardwareInspection) => void;
  onError: (error?: string) => void;
  onOpenMotionSettings: () => void;
  maxJogDistanceMm: number;
  maxJogFeedMmPerMin: number;
  useProbeForZ: boolean;
  homingInstalled: boolean;
  spindleControl: SpindleControl;
  floodCoolantControl: boolean;
  mistCoolantControl: boolean;
  activeCoordinateSystem: WorkCoordinateSystem;
  rotaryAxis?: RotaryAxisProfile;
}

export function SafetyControls({
  snapshot,
  desktopRuntime,
  extensionRegistry,
  machineGateway,
  workCoordinateGateway,
  machineBound,
  onSnapshot,
  onInspection,
  onError,
  onOpenMotionSettings,
  maxJogDistanceMm,
  maxJogFeedMmPerMin,
  useProbeForZ,
  homingInstalled,
  spindleControl,
  floodCoolantControl,
  mistCoolantControl,
  activeCoordinateSystem,
  rotaryAxis,
}: SafetyControlsProps) {
  return (
    <section className="safety-controls" aria-label="Ручное управление">
      <MachineReferencePanel
        desktopRuntime={desktopRuntime}
        disabled={!machineBound}
        homingInstalled={homingInstalled}
        onError={onError}
        onSnapshot={onSnapshot}
        snapshot={snapshot}
      />

      <UiExtensionSlot
        context={{
          snapshot,
          desktopRuntime,
          controlsDisabled: !machineBound,
          machineCommands: machineGateway,
          workCoordinates: workCoordinateGateway,
          updateSnapshot: onSnapshot,
          updateInspection: onInspection,
          reportError: onError,
          openControllerMotionSettings: onOpenMotionSettings,
          maxJogDistanceMm,
          maxJogFeedMmPerMin,
          useProbeForZ,
          rotaryAxis,
        }}
        onExtensionError={(contributionId, error) =>
          onError(`Plugin UI ${contributionId}: ${String(error)}`)
        }
        registry={extensionRegistry}
        slot={uiSlots.controlMachine}
      />
      <details className="control-disclosure coordinate-disclosure">
        <summary>
          <span>Рабочий ноль</span>
          <ChevronDown aria-hidden="true" size={14} />
        </summary>
        <UiExtensionSlot
          context={{
            snapshot,
            desktopRuntime,
            controlsDisabled: !machineBound,
            machineCommands: machineGateway,
            workCoordinates: workCoordinateGateway,
            updateSnapshot: onSnapshot,
            updateInspection: onInspection,
            reportError: onError,
            openControllerMotionSettings: onOpenMotionSettings,
            maxJogDistanceMm,
            maxJogFeedMmPerMin,
            useProbeForZ,
            rotaryAxis,
          }}
          onExtensionError={(contributionId, error) =>
            onError(`Plugin UI ${contributionId}: ${String(error)}`)
          }
          registry={extensionRegistry}
          slot={uiSlots.controlCoordinates}
        />
      </details>
      <details className="control-disclosure machine-setup-disclosure">
        <summary>
          <span>G54–G59 и выходы</span>
          <ChevronDown aria-hidden="true" size={14} />
        </summary>
        <MachineSetupPanel
          activeCoordinateSystem={activeCoordinateSystem}
          disabled={!machineBound}
          onError={onError}
          onSnapshot={onSnapshot}
          snapshot={snapshot}
          spindleControl={spindleControl}
          floodCoolantControl={floodCoolantControl}
          mistCoolantControl={mistCoolantControl}
        />
      </details>
    </section>
  );
}
