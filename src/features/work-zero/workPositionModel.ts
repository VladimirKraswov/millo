import type {
  ControllerSnapshot,
  HardwareInspection,
  Position,
} from "../../shared/machine";

export interface WorkPositionView {
  readonly coordinateSystem: string;
  readonly position?: Position;
}

const coordinateSystems = new Set(["G54", "G55", "G56", "G57", "G58", "G59"]);

const parseVector = (value: string | undefined): Position | undefined => {
  if (!value) return undefined;
  const parts = value.split(":")[0].split(",").map(Number);
  if (parts.length < 3 || parts.slice(0, 3).some((part) => !Number.isFinite(part))) {
    return undefined;
  }
  return { x: parts[0], y: parts[1], z: parts[2] };
};

const addOffsets = (left: Position, right?: Position): Position => ({
  x: left.x + (right?.x ?? 0),
  y: left.y + (right?.y ?? 0),
  z: left.z + (right?.z ?? 0),
});

export function resolveWorkPosition(
  snapshot: ControllerSnapshot,
  inspection?: HardwareInspection,
): WorkPositionView {
  const coordinateSystem =
    inspection?.device.modalState.find((word) => coordinateSystems.has(word.toUpperCase()))
      ?.toUpperCase() ?? "G54";
  if (snapshot.machine.workPosition) {
    return { coordinateSystem, position: snapshot.machine.workPosition };
  }
  const machinePosition = snapshot.machine.machinePosition;
  if (!machinePosition) return { coordinateSystem };

  let offset = snapshot.machine.workCoordinateOffset;
  if (!offset && inspection) {
    const workOffset = parseVector(inspection.device.parameters[coordinateSystem]);
    if (workOffset) {
      offset = addOffsets(workOffset, parseVector(inspection.device.parameters.G92));
      const toolLength = Number(inspection.device.parameters.TLO);
      if (Number.isFinite(toolLength)) {
        offset = { ...offset, z: offset.z + toolLength };
      }
    }
  }
  if (!offset) return { coordinateSystem };

  return {
    coordinateSystem,
    position: {
      x: machinePosition.x - offset.x,
      y: machinePosition.y - offset.y,
      z: machinePosition.z - offset.z,
      ...(machinePosition.a === undefined ? {} : { a: machinePosition.a }),
    },
  };
}
