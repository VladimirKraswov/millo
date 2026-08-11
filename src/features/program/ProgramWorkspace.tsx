import {
  Box,
  FileCode2,
  Pause,
  Play,
  ShieldAlert,
  Square,
  Trash2,
  TriangleAlert,
  Upload,
  X,
} from "lucide-react";
import {
  lazy,
  Suspense,
  useEffect,
  useMemo,
  useState,
  type ChangeEvent,
  type DragEvent,
} from "react";

import type { ProgramGateway } from "../../platform/program/ProgramGateway";
import {
  idleSenderSnapshot,
  type DryRunGateway,
  type SenderSnapshot,
  type SenderState,
} from "../../shared/dryRun";
import type { GcodeProgram, ProgramWarning } from "../../shared/program";
import { ProgramLoader, type LoadedProgram } from "./ProgramLoader";
import { dryRunControls } from "./dryRunReadModel";
import type { PreviewView } from "./ToolpathPreview";

const ToolpathPreview = lazy(async () => {
  const module = await import("./ToolpathPreview");
  return { default: module.ToolpathPreview };
});

interface ProgramWorkspaceProps {
  readonly desktopRuntime: boolean;
  readonly dryRunAvailable?: boolean;
  readonly dryRunGateway?: DryRunGateway;
  readonly gateway: ProgramGateway;
  readonly initialProgram?: GcodeProgram;
  readonly initialSource?: string;
}

const formatDistance = (value: number): string =>
  value >= 1_000 ? `${(value / 1_000).toFixed(2)} m` : `${value.toFixed(1)} mm`;

const warningTitle = (warning: ProgramWarning): string =>
  warning.code.replaceAll("-", " ");

const senderLabels: Record<SenderState, string> = {
  idle: "Not started",
  ready: "Ready",
  running: "Running on Mock",
  paused: "Paused",
  completed: "Completed",
  failed: "Stopped on error",
  cancelled: "Cancelled",
};

export function ProgramWorkspace({
  desktopRuntime,
  dryRunAvailable = false,
  dryRunGateway,
  gateway,
  initialProgram,
  initialSource = "",
}: ProgramWorkspaceProps) {
  const loader = useMemo(() => new ProgramLoader(gateway), [gateway]);
  const [loaded, setLoaded] = useState<LoadedProgram | undefined>(
    initialProgram ? { program: initialProgram, source: initialSource } : undefined,
  );
  const [sender, setSender] = useState<SenderSnapshot>(idleSenderSnapshot);
  const [view, setView] = useState<PreviewView>("iso");
  const [loading, setLoading] = useState(false);
  const [dragging, setDragging] = useState(false);
  const [error, setError] = useState<string>();
  const program = loaded?.program;
  const senderActive = sender.state === "running" || sender.state === "paused";

  useEffect(() => {
    if (!desktopRuntime || !dryRunGateway) return;
    let active = true;
    let unsubscribe: (() => void) | undefined;
    void dryRunGateway
      .snapshot()
      .then((snapshot) => {
        if (active) setSender(snapshot);
      })
      .catch((reason: unknown) => {
        if (active) setError(String(reason));
      });
    void dryRunGateway
      .subscribe((snapshot) => {
        if (active) setSender(snapshot);
      })
      .then((stop) => {
        if (active) unsubscribe = stop;
        else stop();
      })
      .catch((reason: unknown) => {
        if (active) setError(String(reason));
      });
    return () => {
      active = false;
      unsubscribe?.();
    };
  }, [desktopRuntime, dryRunGateway]);

  const loadFile = async (file?: File) => {
    if (!file || loading || !desktopRuntime) return;
    setLoading(true);
    setError(undefined);
    try {
      setLoaded(await loader.load(file));
      setSender(idleSenderSnapshot);
    } catch (reason) {
      setError(String(reason));
    } finally {
      setLoading(false);
    }
  };

  const selectFile = (event: ChangeEvent<HTMLInputElement>) => {
    const selected = event.currentTarget.files?.[0];
    event.currentTarget.value = "";
    void loadFile(selected);
  };

  const dropFile = (event: DragEvent<HTMLDivElement>) => {
    event.preventDefault();
    setDragging(false);
    void loadFile(event.dataTransfer.files[0]);
  };

  const bounds = program?.summary.bounds;
  const pathDistance = program
    ? program.summary.rapidDistanceMm + program.summary.cuttingDistanceMm
    : 0;
  const runSenderAction = async (action: () => Promise<SenderSnapshot>) => {
    setError(undefined);
    try {
      setSender(await action());
    } catch (reason) {
      setError(String(reason));
    }
  };
  const startDryRun = () => {
    if (!loaded || !dryRunGateway) return;
    void runSenderAction(() =>
      dryRunGateway.start({
        sourceName: loaded.program.sourceName,
        source: loaded.source,
      }),
    );
  };
  const senderForProgram = sender.sourceName === program?.sourceName;
  const displayedSender = senderForProgram ? sender : idleSenderSnapshot;
  const controls = dryRunControls(displayedSender, {
    mockAvailable: dryRunAvailable,
    policyEligible: program?.summary.dryRunEligible ?? false,
    loading,
  });
  const progressPercent = controls.progressPercent;

  return (
    <section className="program-workspace" aria-labelledby="program-title">
      <header className="program-header">
        <div className="program-identity">
          <span>Program</span>
          <strong id="program-title">{program?.sourceName ?? "G-code preview"}</strong>
        </div>
        <div className="program-actions">
          {program && (
            <div className="preview-view" role="group" aria-label="Preview view">
              <button
                aria-label="Top view"
                aria-pressed={view === "top"}
                onClick={() => setView("top")}
                title="Top view"
                type="button"
              >
                <Square aria-hidden="true" size={14} />
              </button>
              <button
                aria-label="Isometric view"
                aria-pressed={view === "iso"}
                onClick={() => setView("iso")}
                title="Isometric view"
                type="button"
              >
                <Box aria-hidden="true" size={14} />
              </button>
            </div>
          )}
          {program && (
            <button
              aria-label="Закрыть программу"
              className="program-icon-action"
              disabled={senderActive}
              onClick={() => {
                setLoaded(undefined);
                setSender(idleSenderSnapshot);
                setError(undefined);
              }}
              title="Закрыть программу"
              type="button"
            >
              <Trash2 aria-hidden="true" size={14} />
            </button>
          )}
          <label className={`program-load${loading ? " is-loading" : ""}`}>
            <Upload aria-hidden="true" size={14} />
            <span>{loading ? "Разбор..." : "Загрузить"}</span>
            <input
              accept=".nc,.ngc,.gcode,.tap,.cnc"
              disabled={!desktopRuntime || loading || senderActive}
              onChange={selectFile}
              type="file"
            />
          </label>
        </div>
      </header>

      {program ? (
        <div className="program-body">
          <div className="program-preview-stage">
            <Suspense
              fallback={<div className="toolpath-preview is-loading">Preview...</div>}
            >
              <ToolpathPreview program={program} view={view} />
            </Suspense>
            <div className="preview-legend" aria-label="Toolpath legend">
              <span className="is-cut">Cut</span>
              <span className="is-rapid">Rapid</span>
            </div>
            <dl className="program-metrics">
              <div>
                <dt>Lines</dt>
                <dd>{program.summary.lineCount}</dd>
              </div>
              <div>
                <dt>Motions</dt>
                <dd>{program.summary.motionCount}</dd>
              </div>
              <div>
                <dt>Path</dt>
                <dd>{formatDistance(pathDistance)}</dd>
              </div>
              <div>
                <dt>Size XYZ</dt>
                <dd>
                  {bounds
                    ? `${bounds.size.x.toFixed(1)} × ${bounds.size.y.toFixed(1)} × ${bounds.size.z.toFixed(1)}`
                    : "--"}
                </dd>
              </div>
            </dl>
          </div>

          <aside className="program-diagnostics" aria-label="Program diagnostics">
            <div
              className={`program-gate ${program.summary.dryRunEligible ? "is-clear" : "is-blocked"}`}
            >
              {program.summary.dryRunEligible ? (
                <FileCode2 aria-hidden="true" size={16} />
              ) : (
                <ShieldAlert aria-hidden="true" size={16} />
              )}
              <div>
                <span>Safety gate</span>
                <strong>
                  {program.summary.dryRunEligible
                    ? "Geometry ready"
                    : "Review required"}
                </strong>
              </div>
            </div>
            <div className={`dry-run-card is-${displayedSender.state}`}>
              <div className="dry-run-heading">
                <div>
                  <span>Bounded sender</span>
                  <strong>{senderLabels[displayedSender.state]}</strong>
                </div>
                <code>{progressPercent}%</code>
              </div>
              <div
                aria-label="Dry run progress"
                aria-valuemax={100}
                aria-valuemin={0}
                aria-valuenow={progressPercent}
                className="dry-run-progress"
                role="progressbar"
              >
                <i style={{ width: `${progressPercent}%` }} />
              </div>
              <div className="dry-run-line">
                <span>
                  {displayedSender.currentSourceLine !== undefined
                    ? `L${displayedSender.currentSourceLine}`
                    : "Guard"}
                </span>
                <code>{displayedSender.currentCommand ?? "M5 · M9 preamble"}</code>
              </div>
              <div className="dry-run-actions">
                {!senderActive && displayedSender.state !== "running" && (
                  <button
                    disabled={!dryRunGateway || !controls.canStart}
                    onClick={startDryRun}
                    type="button"
                  >
                    <Play aria-hidden="true" size={13} />
                    Mock dry run
                  </button>
                )}
                {displayedSender.state === "running" && dryRunGateway && (
                  <button
                    onClick={() => void runSenderAction(dryRunGateway.pause)}
                    type="button"
                  >
                    <Pause aria-hidden="true" size={13} />
                    Pause
                  </button>
                )}
                {displayedSender.state === "paused" && dryRunGateway && (
                  <button
                    disabled={!controls.canResume}
                    onClick={() => void runSenderAction(dryRunGateway.resume)}
                    type="button"
                  >
                    <Play aria-hidden="true" size={13} />
                    Resume
                  </button>
                )}
                {senderActive && dryRunGateway && (
                  <button
                    className="is-cancel"
                    onClick={() => void runSenderAction(dryRunGateway.cancel)}
                    type="button"
                  >
                    <X aria-hidden="true" size={13} />
                    Cancel
                  </button>
                )}
              </div>
              {!dryRunAvailable && (
                <small>Подключите Mock GRBL в состоянии Idle</small>
              )}
              {displayedSender.lastError && (
                <small className="is-error">{displayedSender.lastError}</small>
              )}
            </div>
            <div className="warning-heading">
              <span>Warnings</span>
              <strong>{program.warnings.length}</strong>
            </div>
            <div className="program-warnings">
              {program.warnings.length === 0 ? (
                <div className="warnings-empty">Parser warnings отсутствуют</div>
              ) : (
                program.warnings.map((warning, index) => (
                  <div
                    className={`program-warning is-${warning.severity}`}
                    key={`${warning.sourceLine}-${warning.code}-${index}`}
                  >
                    <span className="warning-line">L{warning.sourceLine}</span>
                    {warning.severity === "safety" ? (
                      <ShieldAlert aria-hidden="true" size={13} />
                    ) : (
                      <TriangleAlert aria-hidden="true" size={13} />
                    )}
                    <div>
                      <strong>{warningTitle(warning)}</strong>
                      <span>{warning.message}</span>
                    </div>
                  </div>
                ))
              )}
            </div>
          </aside>
        </div>
      ) : senderActive && dryRunGateway ? (
        <div className="program-dropzone sender-recovery" role="status">
          <ShieldAlert aria-hidden="true" size={28} />
          <strong>{sender.sourceName ?? "Mock dry run"}</strong>
          <span>{senderLabels[sender.state]}</span>
          <button
            onClick={() => void runSenderAction(dryRunGateway.cancel)}
            type="button"
          >
            <X aria-hidden="true" size={13} />
            Cancel sender
          </button>
        </div>
      ) : (
        <div
          className={`program-dropzone${dragging ? " is-dragging" : ""}`}
          onDragEnter={(event) => {
            event.preventDefault();
            if (desktopRuntime) setDragging(true);
          }}
          onDragLeave={() => setDragging(false)}
          onDragOver={(event) => event.preventDefault()}
          onDrop={dropFile}
        >
          <FileCode2 aria-hidden="true" size={28} />
          <strong>Программа не загружена</strong>
          <span>.nc · .ngc · .gcode · .tap · .cnc</span>
        </div>
      )}

      {error && <p className="program-error">{error}</p>}
    </section>
  );
}
