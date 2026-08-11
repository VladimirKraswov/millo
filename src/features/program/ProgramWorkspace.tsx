import {
  Box,
  CircleAlert,
  CircleCheck,
  FileCode2,
  Pause,
  Play,
  RefreshCw,
  ScanSearch,
  ShieldAlert,
  ShieldCheck,
  Square,
  Trash2,
  TriangleAlert,
  Upload,
  Wrench,
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
import type { HardwareInspection } from "../../shared/machine";
import {
  idleSenderSnapshot,
  type DryRunGateway,
  type SenderSnapshot,
  type SenderState,
} from "../../shared/dryRun";
import type { GcodeProgram, ProgramWarning } from "../../shared/program";
import type {
  FirstCutConfirmation,
  FirstCutPreparation,
  ProgramRunIntent,
  RealRunPreflightGateway,
  RunPreflightReport,
  ToolChangeConfirmation,
} from "../../shared/realRun";
import { FirstCutAuthorizationDialog } from "./FirstCutAuthorizationDialog";
import { ProgramLoader, type LoadedProgram } from "./ProgramLoader";
import { ProgramLineTable } from "./ProgramLineTable";
import { ToolChangeDialog } from "./ToolChangeDialog";
import { canStartCheckRun } from "./checkRunReadModel";
import {
  dryRunControls,
  senderFailureSummary,
  senderTiming,
} from "./dryRunReadModel";
import { realRunPreflightControls } from "./realRunPreflightReadModel";
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
  readonly initialSender?: SenderSnapshot;
  readonly initialSource?: string;
  readonly onInspection?: (inspection: HardwareInspection) => void;
  readonly realRunAvailable?: boolean;
  readonly realRunGateway?: RealRunPreflightGateway;
  readonly realRunTarget?: boolean;
}

const formatDistance = (value: number): string =>
  value >= 1_000 ? `${(value / 1_000).toFixed(2)} m` : `${value.toFixed(1)} mm`;

const formatDuration = (seconds: number, complete: boolean): string => {
  const rounded = Math.max(0, Math.round(seconds));
  const hours = Math.floor(rounded / 3_600);
  const minutes = Math.floor((rounded % 3_600) / 60);
  const remainder = rounded % 60;
  const value = hours > 0
    ? `${hours}h ${minutes}m`
    : minutes > 0
      ? `${minutes}m ${remainder}s`
      : `${remainder}s`;
  return `${complete ? "~" : ">="}${value}`;
};

function SenderTiming({ sender }: { readonly sender: SenderSnapshot }) {
  const timing = senderTiming(sender);
  return (
    <div className="sender-timing" aria-label="Run timing">
      <span>
        Elapsed <code>{timing.elapsed}</code>
      </span>
      <span>
        {timing.estimateLabel} <code>{timing.remaining}</code>
      </span>
    </div>
  );
}

const warningTitle = (warning: ProgramWarning): string =>
  warning.code.replaceAll("-", " ");

const senderLabels: Record<SenderState, string> = {
  idle: "Not started",
  ready: "Ready",
  running: "Running",
  paused: "Paused",
  toolChange: "Tool change",
  draining: "Physical motion",
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
  initialSender,
  initialSource = "",
  onInspection,
  realRunAvailable = false,
  realRunGateway,
  realRunTarget = false,
}: ProgramWorkspaceProps) {
  const loader = useMemo(() => new ProgramLoader(gateway), [gateway]);
  const [loaded, setLoaded] = useState<LoadedProgram | undefined>(
    initialProgram ? { program: initialProgram, source: initialSource } : undefined,
  );
  const [sender, setSender] = useState<SenderSnapshot>(
    initialSender ?? idleSenderSnapshot,
  );
  const [view, setView] = useState<PreviewView>("iso");
  const [loading, setLoading] = useState(false);
  const [dragging, setDragging] = useState(false);
  const [diagnosticView, setDiagnosticView] = useState<
    "lines" | "warnings" | "preflight"
  >("lines");
  const [selectedSourceLine, setSelectedSourceLine] = useState<number>();
  const [realRunReport, setRealRunReport] = useState<RunPreflightReport>();
  const [programRunIntent, setProgramRunIntent] =
    useState<ProgramRunIntent>("airRun");
  const [firstCutOpen, setFirstCutOpen] = useState(false);
  const [toolChangeOpen, setToolChangeOpen] = useState(false);
  const [firstCutPreparation, setFirstCutPreparation] =
    useState<FirstCutPreparation>();
  const [preflightLoading, setPreflightLoading] = useState(false);
  const [error, setError] = useState<string>();
  const program = loaded?.program;
  const senderActive = ["running", "paused", "toolChange", "draining"].includes(
    sender.state,
  );

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

  useEffect(() => {
    if (!realRunTarget || !realRunAvailable) {
      setRealRunReport(undefined);
      setFirstCutPreparation(undefined);
      setFirstCutOpen(false);
      setToolChangeOpen(false);
    }
  }, [realRunAvailable, realRunTarget]);

  useEffect(() => {
    if (sender.state === "toolChange") {
      setToolChangeOpen(true);
    } else {
      setToolChangeOpen(false);
    }
  }, [sender.currentSourceLine, sender.requestedTool, sender.state]);

  const loadFile = async (file?: File) => {
    if (!file || loading || !desktopRuntime) return;
    setLoading(true);
    setError(undefined);
    try {
      setLoaded(await loader.load(file));
      setSender(idleSenderSnapshot);
      setSelectedSourceLine(undefined);
      setDiagnosticView("lines");
      setRealRunReport(undefined);
      setFirstCutPreparation(undefined);
      setFirstCutOpen(false);
      setToolChangeOpen(false);
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
  const motionSourceLines = useMemo(
    () => new Set(program?.toolpath.map((segment) => segment.sourceLine) ?? []),
    [program],
  );
  const selectedProgramLine = useMemo(
    () =>
      program?.lines.find((line) => line.sourceLine === selectedSourceLine),
    [program, selectedSourceLine],
  );
  const selectedMotionCount = useMemo(
    () =>
      selectedSourceLine === undefined
        ? 0
        : (program?.toolpath.filter(
            (segment) => segment.sourceLine === selectedSourceLine,
          ).length ?? 0),
    [program, selectedSourceLine],
  );
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
  const displayedSenderFailure = senderFailureSummary(displayedSender);
  const controls = dryRunControls(displayedSender, {
    mockAvailable: dryRunAvailable,
    policyEligible: program?.summary.dryRunEligible ?? false,
    loading,
  });
  const progressPercent = controls.progressPercent;
  const reportForProgram =
    realRunReport &&
    realRunReport.sourceName === program?.sourceName &&
    realRunReport.intent === programRunIntent
      ? realRunReport
      : undefined;
  const preflightControls = realRunPreflightControls(reportForProgram, {
    serialAvailable: realRunAvailable,
    gatewayAvailable: realRunGateway !== undefined,
    checking: preflightLoading,
  });
  const runRealPreflight = async () => {
    if (!loaded || !realRunGateway || !preflightControls.canCheck) return;
    setPreflightLoading(true);
    setError(undefined);
    setRealRunReport(undefined);
    setFirstCutPreparation(undefined);
    try {
      const report = await realRunGateway.preflight(
        {
          sourceName: loaded.program.sourceName,
          source: loaded.source,
        },
        programRunIntent,
      );
      setRealRunReport(report);
      onInspection?.(report.hardware);
      setDiagnosticView("preflight");
    } catch (reason) {
      setError(String(reason));
    } finally {
      setPreflightLoading(false);
    }
  };
  const authorizeFirstCut = async (
    confirmation: FirstCutConfirmation,
  ): Promise<FirstCutPreparation> => {
    if (!loaded || !realRunGateway) {
      throw new Error("First-cut gateway is unavailable");
    }
    return realRunGateway.authorizeFirstCut(
      {
        sourceName: loaded.program.sourceName,
        source: loaded.source,
      },
      confirmation,
    );
  };
  const startProgramRun = async (
    preparation: FirstCutPreparation,
  ): Promise<SenderSnapshot> => {
    if (!loaded || !realRunGateway) {
      throw new Error("Program-run gateway is unavailable");
    }
    return realRunGateway.startProgram(
      {
        sourceName: loaded.program.sourceName,
        source: loaded.source,
      },
      preparation.authorization.id,
    );
  };
  const startCheckRun = () => {
    if (!loaded || !realRunGateway) return;
    void runSenderAction(() =>
      realRunGateway.startCheck({
        sourceName: loaded.program.sourceName,
        source: loaded.source,
      }),
    );
  };
  const completeToolChange = async (
    confirmation: ToolChangeConfirmation,
  ): Promise<void> => {
    if (!realRunGateway) {
      throw new Error("Tool-change gateway is unavailable");
    }
    setSender(await realRunGateway.completeToolChange(confirmation));
  };
  const programRunVisible =
    senderForProgram && (sender.mode === "airRun" || sender.mode === "cutRun");
  const checkRunVisible = senderForProgram && sender.mode === "checkRun";
  const checkRunAvailable = canStartCheckRun(displayedSender, {
    gatewayAvailable: realRunGateway !== undefined,
    loading,
    programLoaded: loaded !== undefined,
    serialAvailable: realRunAvailable,
  });

  useEffect(() => {
    if ((programRunVisible || checkRunVisible) && sender.currentSourceLine !== undefined) {
      setSelectedSourceLine(sender.currentSourceLine);
    }
  }, [checkRunVisible, programRunVisible, sender.currentSourceLine]);

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
                setSelectedSourceLine(undefined);
                setRealRunReport(undefined);
                setFirstCutPreparation(undefined);
                setFirstCutOpen(false);
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
              <ToolpathPreview
                program={program}
                selectedSourceLine={selectedSourceLine}
                view={view}
              />
            </Suspense>
            <div className="preview-legend" aria-label="Toolpath legend">
              <span className="is-cut">Cut</span>
              <span className="is-rapid">Rapid</span>
            </div>
            {selectedProgramLine && (
              <div className="preview-selection" role="status">
                <span>L{selectedProgramLine.sourceLine}</span>
                <code title={selectedProgramLine.source}>
                  {selectedProgramLine.source || "Empty line"}
                </code>
                <small>
                  {selectedMotionCount > 0
                    ? `${selectedMotionCount} preview segment${selectedMotionCount === 1 ? "" : "s"}`
                    : "No preview motion"}
                </small>
                <button
                  aria-label="Очистить выбор строки"
                  onClick={() => setSelectedSourceLine(undefined)}
                  title="Очистить выбор строки"
                  type="button"
                >
                  <X aria-hidden="true" size={12} />
                </button>
              </div>
            )}
            <dl className="program-metrics">
              <div>
                <dt>Lines</dt>
                <dd>{program.summary.lineCount}</dd>
              </div>
              <div>
                <dt>Time</dt>
                <dd>
                  {formatDuration(
                    program.summary.estimatedTotalTimeSeconds,
                    program.summary.timeEstimateComplete,
                  )}
                </dd>
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
            {realRunTarget && (programRunVisible || checkRunVisible) ? (
              <div className={`dry-run-card program-run-card is-${displayedSender.state}`}>
                <div className="dry-run-heading">
                  <div>
                    <span>
                      {checkRunVisible
                        ? "GRBL Check"
                        : sender.mode === "airRun"
                          ? "Air run"
                          : "Cut run"}
                    </span>
                    <strong>
                      {displayedSender.state === "draining"
                        ? "Waiting for GRBL Idle"
                        : senderLabels[displayedSender.state]}
                    </strong>
                  </div>
                  <code>{progressPercent}%</code>
                </div>
                <div
                  aria-label="Program run progress"
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
                <SenderTiming sender={displayedSender} />
                <div className="dry-run-actions">
                  {displayedSender.state === "paused" && realRunGateway && (
                    <button
                      onClick={() =>
                        void runSenderAction(realRunGateway.resumeProgram)
                      }
                      type="button"
                    >
                      <Play aria-hidden="true" size={13} />
                      Resume
                    </button>
                  )}
                  {displayedSender.state === "toolChange" && realRunGateway && (
                    <button
                      onClick={() => setToolChangeOpen(true)}
                      type="button"
                    >
                      <Wrench aria-hidden="true" size={13} />
                      Подтвердить замену
                    </button>
                  )}
                </div>
                <small>
                  {displayedSender.state === "completed"
                    ? checkRunVisible
                      ? "Все строки приняты в $C; контроллер вернулся в Idle"
                      : "Все строки подтверждены; контроллер вернулся в Idle"
                    : displayedSender.state === "failed"
                      ? displayedSenderFailure
                      : displayedSender.state === "toolChange"
                        ? `M6 удерживается приложением${displayedSender.requestedTool === undefined ? "" : ` · требуется T${displayedSender.requestedTool}`}`
                      : checkRunVisible
                        ? "По одной строке · без движения и включения выходов"
                        : "Остановка: Feed Hold, затем подтверждаемый Soft Reset справа"}
                </small>
              </div>
            ) : realRunTarget ? (
              <div
                className={`real-run-preflight is-${preflightControls.status}`}
              >
                <div
                  aria-label="Режим выполнения"
                  className="program-run-intent"
                  role="group"
                >
                  <button
                    aria-pressed={programRunIntent === "airRun"}
                    disabled={preflightLoading}
                    onClick={() => {
                      setProgramRunIntent("airRun");
                      setRealRunReport(undefined);
                      setFirstCutPreparation(undefined);
                    }}
                    type="button"
                  >
                    Air run
                  </button>
                  <button
                    aria-pressed={programRunIntent === "cutting"}
                    disabled={preflightLoading}
                    onClick={() => {
                      setProgramRunIntent("cutting");
                      setRealRunReport(undefined);
                      setFirstCutPreparation(undefined);
                    }}
                    type="button"
                  >
                    Обработка
                  </button>
                </div>
                <div className="real-run-preflight-heading">
                  <div>
                    <span>Serial preflight</span>
                    <strong>{preflightControls.statusLabel}</strong>
                  </div>
                  {reportForProgram ? (
                    <code>status #{reportForProgram.pollSequence}</code>
                  ) : (
                    <ShieldAlert aria-hidden="true" size={15} />
                  )}
                </div>
                <button
                  disabled={!preflightControls.canCheck}
                  onClick={() => void runRealPreflight()}
                  type="button"
                >
                  <RefreshCw
                    aria-hidden="true"
                    className={preflightLoading ? "is-spinning" : undefined}
                    size={13}
                  />
                  {reportForProgram ? "Проверить снова" : "Проверить готовность"}
                </button>
                <button
                  disabled={!checkRunAvailable}
                  onClick={startCheckRun}
                  title="Проверить файл встроенным режимом GRBL $C без движения"
                  type="button"
                >
                  <ScanSearch aria-hidden="true" size={13} />
                  GRBL Check
                </button>
                {reportForProgram?.ready && (
                  <button
                    className="first-cut-open"
                    onClick={() => setFirstCutOpen(true)}
                    type="button"
                  >
                    <ShieldCheck aria-hidden="true" size={13} />
                    {firstCutPreparation ? "Разрешение выпущено" : "Подтвердить запуск"}
                  </button>
                )}
                <small>
                  {firstCutPreparation
                    ? `Lease #${firstCutPreparation.authorization.id} · максимум 30 секунд · ожидает Start`
                    : reportForProgram?.ready
                    ? `${reportForProgram.cautionCount} caution · готов к авторизации`
                    : reportForProgram
                      ? `${reportForProgram.blockerCount} blocker · ${reportForProgram.cautionCount} caution`
                      : "Проверка файла и свежего состояния GRBL"}
                </small>
              </div>
            ) : (
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
                  <code>
                    {displayedSender.currentCommand ?? "M5 · M9 preamble"}
                  </code>
                </div>
                <SenderTiming sender={displayedSender} />
                <div className="dry-run-actions">
                  {!senderActive && displayedSender.state !== "running" && (
                    <button
                      disabled={!dryRunGateway || !controls.canStart}
                      onClick={startDryRun}
                      title={
                        dryRunAvailable
                          ? "Запустить на Mock GRBL"
                          : "Подключите Mock GRBL в состоянии Idle"
                      }
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
                {displayedSenderFailure && (
                  <small className="is-error">{displayedSenderFailure}</small>
                )}
              </div>
            )}
            <div
              aria-label="Program diagnostics view"
              className={`program-diagnostic-tabs${realRunTarget ? " has-preflight" : ""}`}
              role="tablist"
            >
              <button
                aria-controls="program-lines-panel"
                aria-selected={diagnosticView === "lines"}
                id="program-lines-tab"
                onClick={() => setDiagnosticView("lines")}
                role="tab"
                type="button"
              >
                Lines <strong>{program.lines.length}</strong>
              </button>
              <button
                aria-controls="program-warnings-panel"
                aria-selected={diagnosticView === "warnings"}
                id="program-warnings-tab"
                onClick={() => setDiagnosticView("warnings")}
                role="tab"
                type="button"
              >
                Warnings <strong>{program.warnings.length}</strong>
              </button>
              {realRunTarget && (
                <button
                  aria-controls="program-preflight-panel"
                  aria-selected={diagnosticView === "preflight"}
                  disabled={!reportForProgram}
                  id="program-preflight-tab"
                  onClick={() => setDiagnosticView("preflight")}
                  role="tab"
                  type="button"
                >
                  Preflight <strong>{reportForProgram?.blockerCount ?? "--"}</strong>
                </button>
              )}
            </div>
            <div
              aria-labelledby="program-lines-tab"
              className="program-lines-panel"
              hidden={diagnosticView !== "lines"}
              id="program-lines-panel"
              role="tabpanel"
            >
              <ProgramLineTable
                lines={program.lines}
                motionSourceLines={motionSourceLines}
                onSelect={(sourceLine) =>
                  setSelectedSourceLine((current) =>
                    current === sourceLine ? undefined : sourceLine,
                  )
                }
                selectedSourceLine={selectedSourceLine}
              />
            </div>
            <div
              aria-labelledby="program-warnings-tab"
              className="program-warnings"
              hidden={diagnosticView !== "warnings"}
              id="program-warnings-panel"
              role="tabpanel"
            >
              {program.warnings.length === 0 ? (
                <div className="warnings-empty">Parser warnings отсутствуют</div>
              ) : (
                program.warnings.map((warning, index) => (
                  <button
                    aria-pressed={selectedSourceLine === warning.sourceLine}
                    className={`program-warning is-${warning.severity}`}
                    key={`${warning.sourceLine}-${warning.code}-${index}`}
                    onClick={() => setSelectedSourceLine(warning.sourceLine)}
                    type="button"
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
                  </button>
                ))
              )}
            </div>
            {realRunTarget && (
              <div
                aria-labelledby="program-preflight-tab"
                className="real-run-checks"
                hidden={diagnosticView !== "preflight"}
                id="program-preflight-panel"
                role="tabpanel"
              >
                {reportForProgram?.checks.map((item) => {
                  const sourceLine = item.sourceLine;
                  const content = (
                    <>
                      {item.level === "pass" ? (
                        <CircleCheck aria-hidden="true" size={13} />
                      ) : (
                        <CircleAlert aria-hidden="true" size={13} />
                      )}
                      <span>
                        <strong>{item.title}</strong>
                        <small>{item.detail}</small>
                      </span>
                      {sourceLine !== undefined && <code>L{sourceLine}</code>}
                    </>
                  );
                  return sourceLine !== undefined ? (
                    <button
                      className={`real-run-check is-${item.level}`}
                      key={item.id}
                      onClick={() => {
                        setSelectedSourceLine(sourceLine);
                        setDiagnosticView("lines");
                      }}
                      type="button"
                    >
                      {content}
                    </button>
                  ) : (
                    <div
                      className={`real-run-check is-${item.level}`}
                      key={item.id}
                    >
                      {content}
                    </div>
                  );
                })}
              </div>
            )}
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

      <FirstCutAuthorizationDialog
        intent={programRunIntent}
        onAuthorize={authorizeFirstCut}
        onAuthorized={(preparation) => {
          setFirstCutPreparation(preparation);
          setRealRunReport(preparation.report);
          onInspection?.(preparation.report.hardware);
        }}
        onClose={() => setFirstCutOpen(false)}
        onStart={startProgramRun}
        onStarted={(snapshot) => {
          setSender(snapshot);
          setFirstCutPreparation(undefined);
        }}
        open={firstCutOpen}
        report={reportForProgram}
      />

      {displayedSender.state === "toolChange" &&
        displayedSender.currentSourceLine !== undefined &&
        realRunGateway && (
          <ToolChangeDialog
            onClose={() => setToolChangeOpen(false)}
            onComplete={completeToolChange}
            open={toolChangeOpen}
            requestedTool={displayedSender.requestedTool}
            sourceLine={displayedSender.currentSourceLine}
          />
        )}

      {error && <p className="program-error">{error}</p>}
    </section>
  );
}
