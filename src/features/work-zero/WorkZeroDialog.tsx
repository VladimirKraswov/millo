import { DialogSurface } from "../../components/DialogSurface";
import { Crosshair, X } from "lucide-react";

import type { WorkCoordinateGateway } from "../../platform/machine/WorkCoordinateGateway";
import type { ControllerSnapshot, Position } from "../../shared/machine";
import { WorkZeroPanel } from "./WorkZeroPanel";

interface WorkZeroDialogProps {
  readonly activeCoordinateSystem: string;
  readonly desktopRuntime: boolean;
  readonly disabled?: boolean;
  readonly gateway: WorkCoordinateGateway;
  readonly onClose: () => void;
  readonly onError: (error?: string) => void;
  readonly onSnapshot: (snapshot: ControllerSnapshot) => void;
  readonly open: boolean;
  readonly position?: Position;
  readonly snapshot: ControllerSnapshot;
  readonly useProbeForZ?: boolean;
}

const formatAxis = (value: number | undefined): string =>
  value === undefined ? "--" : value.toFixed(3);

export function WorkZeroDialog({
  activeCoordinateSystem,
  desktopRuntime,
  disabled = false,
  gateway,
  onClose,
  onError,
  onSnapshot,
  open,
  position,
  snapshot,
  useProbeForZ = false,
}: WorkZeroDialogProps) {
  if (!open) return null;

  return (
    <div
      className="machine-dialog-backdrop work-zero-backdrop"
      role="presentation"
    >
      <DialogSurface
        onDismiss={onClose}
        modal={false}
        aria-labelledby="work-zero-dialog-title"
        className="machine-dialog work-zero-dialog"
      >
        <header>
          <div>
            <span>Рабочая система · {activeCoordinateSystem}</span>
            <h2 id="work-zero-dialog-title">Рабочий ноль</h2>
          </div>
          <button
            aria-label="Закрыть"
            onClick={onClose}
            title="Закрыть"
            type="button"
          >
            <X aria-hidden="true" size={16} />
          </button>
        </header>
        <div className="work-zero-dialog-body">
          <div
            className="work-zero-current"
            aria-label="Текущая рабочая позиция"
          >
            <Crosshair aria-hidden="true" size={18} />
            <span>Сейчас</span>
            <code>X {formatAxis(position?.x)}</code>
            <code>Y {formatAxis(position?.y)}</code>
            <code>Z {formatAxis(position?.z)}</code>
          </div>
          <WorkZeroPanel
            activeCoordinateSystem={activeCoordinateSystem}
            desktopRuntime={desktopRuntime}
            disabled={disabled}
            gateway={gateway}
            onError={onError}
            onSnapshot={onSnapshot}
            snapshot={snapshot}
            useProbeForZ={useProbeForZ}
          />
        </div>
      </DialogSurface>
    </div>
  );
}
