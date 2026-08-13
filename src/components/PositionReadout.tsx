import type { Position } from "../shared/machine";

export const formatCoordinate = (value: number | undefined): string =>
  value === undefined ? "--" : value.toFixed(3);

function AxisReadout({ axis, value }: { readonly axis: string; readonly value?: number }) {
  return (
    <div className="axis-readout">
      <span>{axis}</span>
      <strong>{formatCoordinate(value)}</strong>
      <small>mm</small>
    </div>
  );
}

export function PositionReadout({ position }: { readonly position?: Position }) {
  return (
    <div className="position-grid">
      <AxisReadout axis="X" value={position?.x} />
      <AxisReadout axis="Y" value={position?.y} />
      <AxisReadout axis="Z" value={position?.z} />
      {position?.a !== undefined && <AxisReadout axis="A" value={position.a} />}
    </div>
  );
}
