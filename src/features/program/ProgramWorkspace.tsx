import {
  Box,
  ChevronDown,
  CircleAlert,
  CircleCheck,
  FileCode2,
  History,
  Pause,
  Play,
  RotateCcw,
  ScanSearch,
  ShieldAlert,
  SlidersHorizontal,
  Square,
  Trash2,
  TriangleAlert,
  Wrench,
  X,
} from "lucide-react";
import {
  lazy,
  Suspense,
  useEffect,
  useMemo,
  useState,
  type DragEvent,
} from "react";

import type { ProgramGateway } from "../../platform/program/ProgramGateway";
import type {
  ControllerSnapshot,
  HardwareInspection,
  Position,
} from "../../shared/machine";
import {
  idleSenderSnapshot,
  type DryRunGateway,
  type SenderSnapshot,
  type SenderState,
} from "../../shared/dryRun";
import type { GcodeProgram, ProgramWarning } from "../../shared/program";
import type {
  ProgramRecoveryCandidate,
  ProgramRecoveryPackage,
  ProgramRecoveryPreparationRequest,
} from "../../shared/recovery";
import { defaultProgramExecutionOptions } from "../../shared/realRun";
import type {
  FirstCutConfirmation,
  FirstCutPreparation,
  ProgramExecutionOptions,
  ProgramRunIntent,
  RealRunPreflightGateway,
  RunPreflightReport,
  ToolChangeConfirmation,
} from "../../shared/realRun";
import { FirstCutAuthorizationDialog } from "./FirstCutAuthorizationDialog";
import { JobReadinessPanel } from "./JobReadinessPanel";
import { ProgramFilePicker } from "./ProgramFilePicker";
import { ProgramLoader, type LoadedProgram } from "./ProgramLoader";
import { ProgramLineTable } from "./ProgramLineTable";
import { ProgramRecoveryDialog } from "./ProgramRecoveryDialog";
import { ToolChangeDialog } from "./ToolChangeDialog";
import { canStartCheckRun } from "./checkRunReadModel";
import {
  dryRunControls,
  senderFailureSummary,
  senderHeartbeat,
  senderTiming,
} from "./dryRunReadModel";
import { realRunPreflightControls } from "./realRunPreflightReadModel";
import {
  physicalSenderActionLayout,
  senderActionLayout,
  senderRunIsVisibleForProgram,
} from "./operatorLayoutModel";
import {
  jobReadinessModel,
  type JobReadinessAction,
} from "./jobReadinessModel";
import type { PreviewView } from "./ToolpathPreview";

const ToolpathPreview = lazy(async () => {
  const module = await import("./ToolpathPreview");
  return { default: module.ToolpathPreview };
});

export interface ProgramMachineContext {
  readonly activeCoordinateSystem: string;
  readonly busy: boolean;
  readonly machineBound: boolean;
  readonly machineName: string;
  readonly onAcknowledgeReset: () => void | Promise<unknown>;
  readonly onConnect: () => void | Promise<unknown>;
  readonly onOpenWorkZero: () => void;
  readonly onUnlock: () => void | Promise<unknown>;
  readonly snapshot: ControllerSnapshot;
  readonly workPosition?: Position;
}

interface ProgramWorkspaceProps {
  readonly desktopRuntime: boolean;
  readonly dryRunAvailable?: boolean;
  readonly dryRunGateway?: DryRunGateway;
  readonly gateway: ProgramGateway;
  readonly initialProgram?: GcodeProgram;
  readonly initialRunIntent?: ProgramRunIntent;
  readonly initialSender?: SenderSnapshot;
  readonly initialSource?: string;
  readonly machineContext?: ProgramMachineContext;
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
  const heartbeat = senderHeartbeat(sender);
  return (
    <>
      <div className="sender-timing" aria-label="Run timing">
        <span>
          Elapsed <code>{timing.elapsed}</code>
        </span>
        <span>
          {timing.estimateLabel} <code>{timing.remaining}</code>
        </span>
      </div>
      <div
        className="sender-heartbeat"
        aria-label="Sender acknowledgement heartbeat"
      >
        <span>ACK #{heartbeat.sequence}</span>
        <code>
          {heartbeat.lastLine} · {heartbeat.age}
        </code>
        <strong className={heartbeat.shutdownAcknowledged ? undefined : "is-placeholder"}>
          M5 · M9 OK
        </strong>
      </div>
    </>
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
  initialRunIntent = "airRun",
  initialSender,
  initialSource = "",
  machineContext,
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
  const [diagnosticsOpen, setDiagnosticsOpen] = useState(
    (initialProgram?.warnings.length ?? 0) > 0,
  );
  const [selectedSourceLine, setSelectedSourceLine] = useState<number>();
  const [realRunReport, setRealRunReport] = useState<RunPreflightReport>();
  const [programRunIntent, setProgramRunIntent] =
    useState<ProgramRunIntent>(initialRunIntent);
  const [programExecutionOptions, setProgramExecutionOptions] =
    useState<ProgramExecutionOptions>(defaultProgramExecutionOptions);
  const [firstCutOpen, setFirstCutOpen] = useState(false);
  const [toolChangeOpen, setToolChangeOpen] = useState(false);
  const [recoveryOpen, setRecoveryOpen] = useState(false);
  const [recoveryCandidate, setRecoveryCandidate] =
    useState<ProgramRecoveryCandidate>();
  const [recoveryChecked, setRecoveryChecked] = useState(realRunGateway === undefined);
  const [stopConfirming, setStopConfirming] = useState(false);
  const [senderCommandBusy, setSenderCommandBusy] = useState(false);
  const [clearedSenderRunSequence, setClearedSenderRunSequence] = useState<number>();
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
    if (!realRunGateway) {
      setRecoveryChecked(true);
      return;
    }
    let active = true;
    setRecoveryChecked(false);
    setRecoveryCandidate(undefined);
    void realRunGateway
      .recoveryCandidate()
      .then((candidate) => {
        if (active) {
          setRecoveryCandidate(candidate ?? undefined);
          setRecoveryChecked(true);
        }
      })
      .catch((reason: unknown) => {
        if (active) {
          setError(String(reason));
          setRecoveryChecked(true);
        }
      });
    return () => {
      active = false;
    };
  }, [realRunGateway]);

  useEffect(() => {
    if (
      !realRunGateway ||
      (sender.state !== "failed" && sender.state !== "cancelled")
    ) {
      return;
    }
    let active = true;
    setRecoveryChecked(false);
    void realRunGateway
      .recoveryCandidate()
      .then((candidate) => {
        if (active) {
          setRecoveryCandidate(candidate ?? undefined);
          setRecoveryChecked(true);
        }
      })
      .catch((reason: unknown) => {
        if (active) {
          setError(String(reason));
          setRecoveryChecked(true);
        }
      });
    return () => {
      active = false;
    };
  }, [realRunGateway, sender.runSequence, sender.state]);

  useEffect(() => {
    if (!realRunTarget || !realRunAvailable) {
      setRealRunReport(undefined);
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

  useEffect(() => {
    if (!stopConfirming) return;
    const timer = window.setTimeout(() => setStopConfirming(false), 5_000);
    return () => window.clearTimeout(timer);
  }, [stopConfirming]);

  useEffect(() => {
    if (!["running", "paused", "toolChange", "draining"].includes(sender.state)) {
      setStopConfirming(false);
    }
  }, [sender.state]);

  const loadFile = async (file?: File) => {
    if (!file || loading || !desktopRuntime) return;
    setLoading(true);
    setError(undefined);
    try {
      const next = await loader.load(file);
      setLoaded(next);
      setClearedSenderRunSequence(sender.runSequence || undefined);
      setSender(idleSenderSnapshot);
      setSelectedSourceLine(undefined);
      setDiagnosticView(next.program.warnings.length > 0 ? "warnings" : "lines");
      setDiagnosticsOpen(next.program.warnings.length > 0);
      setRealRunReport(undefined);
      setFirstCutOpen(false);
      setToolChangeOpen(false);
    } catch (reason) {
      setError(String(reason));
    } finally {
      setLoading(false);
    }
  };

  const dropFile = (event: DragEvent<HTMLDivElement>) => {
    event.preventDefault();
    setDragging(false);
    void loadFile(event.dataTransfer.files[0]);
  };

  const prepareRecovery = (
    request: ProgramRecoveryPreparationRequest,
  ): Promise<ProgramRecoveryPackage> => {
    if (!realRunGateway) throw new Error("Recovery gateway is unavailable");
    return realRunGateway.prepareRecovery(request);
  };

  const loadRecoveryPackage = async (prepared: ProgramRecoveryPackage) => {
    setLoading(true);
    setError(undefined);
    try {
      const program = await gateway.parse(prepared.request, {
        blockDelete: prepared.executionOptions.blockDelete,
      });
      setLoaded({ program, source: prepared.request.source });
      setProgramRunIntent(prepared.intent);
      setProgramExecutionOptions(prepared.executionOptions);
      setClearedSenderRunSequence(sender.runSequence || undefined);
      setSender(idleSenderSnapshot);
      setSelectedSourceLine(prepared.restartSourceLine);
      setDiagnosticView("lines");
      setDiagnosticsOpen(false);
      setRealRunReport(undefined);
      setFirstCutOpen(false);
      setRecoveryCandidate(undefined);
    } catch (reason) {
      setError(String(reason));
      throw reason;
    } finally {
      setLoading(false);
    }
  };

  const dismissRecovery = async (recoveryId: number) => {
    if (!realRunGateway) throw new Error("Recovery gateway is unavailable");
    await realRunGateway.dismissRecovery(recoveryId);
    setRecoveryCandidate(undefined);
    setRecoveryChecked(true);
    setClearedSenderRunSequence(sender.runSequence || undefined);
    setSender((current) => ({
      ...idleSenderSnapshot,
      runSequence: current.runSequence,
    }));
    setRealRunReport(undefined);
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
    if (senderCommandBusy) return;
    setSenderCommandBusy(true);
    setError(undefined);
    try {
      setSender(await action());
    } catch (reason) {
      setError(String(reason));
    } finally {
      setSenderCommandBusy(false);
    }
  };

  const stopProgramRun = async () => {
    if (!realRunGateway || senderCommandBusy) return;
    if (!stopConfirming) {
      setStopConfirming(true);
      return;
    }
    setSenderCommandBusy(true);
    setError(undefined);
    setRecoveryChecked(false);
    try {
      setSender(await realRunGateway.abortProgram());
      setRealRunReport(undefined);
      const candidate = await realRunGateway.recoveryCandidate();
      setRecoveryCandidate(candidate ?? undefined);
      setRecoveryChecked(true);
      setRecoveryOpen(candidate !== null);
    } catch (reason) {
      setError(String(reason));
      setRecoveryChecked(true);
    } finally {
      setStopConfirming(false);
      setSenderCommandBusy(false);
    }
  };
  const startDryRun = () => {
    if (!loaded || !dryRunGateway) return;
    setDiagnosticsOpen(false);
    void runSenderAction(() =>
      dryRunGateway.start({
        sourceName: loaded.program.sourceName,
        source: loaded.source,
      }),
    );
  };
  const senderForProgram = senderRunIsVisibleForProgram(
    sender,
    program?.sourceName,
    clearedSenderRunSequence,
  );
  const displayedSender = senderForProgram ? sender : idleSenderSnapshot;
  const displayedSenderFailure = senderFailureSummary(displayedSender);
  const controls = dryRunControls(displayedSender, {
    mockAvailable: dryRunAvailable,
    policyEligible: program?.summary.dryRunEligible ?? false,
    loading,
  });
  const progressPercent = controls.progressPercent;
  const updateExecutionOption = async (
    key: keyof ProgramExecutionOptions,
    value: boolean,
  ) => {
    setRealRunReport(undefined);
    if (key !== "blockDelete" || !loaded) {
      setProgramExecutionOptions((current) => ({ ...current, [key]: value }));
      return;
    }

    setLoading(true);
    setError(undefined);
    try {
      const reparsed = await gateway.parse(
        {
          sourceName: loaded.program.sourceName,
          source: loaded.source,
        },
        { blockDelete: value },
      );
      setLoaded({ ...loaded, program: reparsed });
      setProgramExecutionOptions((current) => ({ ...current, [key]: value }));
    } catch (reason) {
      setError(String(reason));
    } finally {
      setLoading(false);
    }
  };
  const reportForProgram =
    realRunReport &&
    realRunReport.sourceName === program?.sourceName &&
    realRunReport.intent === programRunIntent &&
    realRunReport.executionOptions.optionalStop ===
      programExecutionOptions.optionalStop &&
    realRunReport.executionOptions.blockDelete ===
      programExecutionOptions.blockDelete
      ? realRunReport
      : undefined;
  const preflightControls = realRunPreflightControls(reportForProgram, {
    serialAvailable: realRunAvailable,
    gatewayAvailable: realRunGateway !== undefined,
    checking: preflightLoading,
  });
  const requiresGrblCheck =
    reportForProgram?.checks.some(
      (check) => check.id === "grbl-check-certificate" && check.level === "blocker",
    ) ?? false;
  const readiness = jobReadinessModel({
    alarm: machineContext?.snapshot.alarm !== undefined,
    connection: machineContext?.snapshot.connection ?? "disconnected",
    machineBound: machineContext?.machineBound ?? false,
    machineMode: machineContext?.snapshot.machine.mode ?? "unknown",
    parserEligible: program?.summary.dryRunEligible ?? false,
    preflightStatus: preflightControls.status,
    resetPending: machineContext?.snapshot.resetNotice !== undefined,
    recoveryStatus: !recoveryChecked
      ? "checking"
      : recoveryCandidate
        ? "outstanding"
        : "clear",
    requiresGrblCheck,
    workPositionAvailable: machineContext?.workPosition !== undefined,
  });
  const machineDetail = machineContext
    ? machineContext.snapshot.connection !== "connected"
      ? "Не подключен"
      : machineContext.snapshot.alarm
        ? `ALARM${machineContext.snapshot.alarm.code === undefined ? "" : `:${machineContext.snapshot.alarm.code}`}`
        : machineContext.snapshot.resetNotice
          ? "Контроллер перезапущен"
          : `${machineContext.machineName} · ${machineContext.snapshot.machine.reportedMode}`
    : "Не подключен";
  const fileDetail = program?.summary.dryRunEligible
    ? `${program.summary.lineCount} строк · ${program.warnings.length} замечаний`
    : `${program?.warnings.length ?? 0} замечаний требуют внимания`;
  const originDetail = machineContext?.workPosition
    ? `${machineContext.activeCoordinateSystem} · X ${machineContext.workPosition.x.toFixed(3)} · Y ${machineContext.workPosition.y.toFixed(3)} · Z ${machineContext.workPosition.z.toFixed(3)}`
    : `${machineContext?.activeCoordinateSystem ?? "G54"} · не установлен`;
  const validationDetail =
    !recoveryChecked
      ? "Проверяем историю запусков"
      : recoveryCandidate
        ? `Нужно решение: ${recoveryCandidate.sourceName}`
        : preflightControls.status === "ready"
          ? `Готово · ${reportForProgram?.cautionCount ?? 0} замечаний`
          : preflightControls.status === "blocked"
            ? requiresGrblCheck
              ? "Нужна проверка контроллером"
              : `${reportForProgram?.blockerCount ?? 0} блокирующих замечаний`
            : preflightControls.status === "checking"
              ? "Читаем состояние GRBL"
              : "Еще не выполнялась";
  const runRealPreflight = async () => {
    if (!loaded || !realRunGateway || !preflightControls.canCheck) return;
    setPreflightLoading(true);
    setError(undefined);
    setRealRunReport(undefined);
    try {
      const report = await realRunGateway.preflight(
        {
          sourceName: loaded.program.sourceName,
          source: loaded.source,
        },
        programRunIntent,
        programExecutionOptions,
      );
      setRealRunReport(report);
      onInspection?.(report.hardware);
      setDiagnosticView("preflight");
      setDiagnosticsOpen(!report.ready);
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
    try {
      return await realRunGateway.startProgram(
        {
          sourceName: loaded.program.sourceName,
          source: loaded.source,
        },
        preparation.authorization.id,
        programExecutionOptions,
      );
    } catch (reason) {
      if (String(reason).includes("unfinished recovery record")) {
        try {
          const candidate = await realRunGateway.recoveryCandidate();
          setRecoveryCandidate(candidate ?? undefined);
          setRecoveryChecked(true);
          setFirstCutOpen(false);
          setRecoveryOpen(candidate !== null);
        } catch {
          setRecoveryChecked(true);
        }
      }
      throw reason;
    }
  };
  const startCheckRun = () => {
    if (!loaded || !realRunGateway) return;
    setDiagnosticsOpen(false);
    void runSenderAction(() =>
      realRunGateway.startCheck(
        {
          sourceName: loaded.program.sourceName,
          source: loaded.source,
        },
        programExecutionOptions,
      ),
    );
  };
  const runReadinessAction = (action: JobReadinessAction) => {
    if (action === "connect") void machineContext?.onConnect();
    if (action === "unlock") void machineContext?.onUnlock();
    if (action === "acknowledgeReset") void machineContext?.onAcknowledgeReset();
    if (action === "setWorkZero") machineContext?.onOpenWorkZero();
    if (action === "runPreflight") void runRealPreflight();
    if (action === "runGrblCheck") {
      setRealRunReport(undefined);
      startCheckRun();
    }
    if (action === "resolveRecovery") setRecoveryOpen(true);
    if (action === "startProgram") setFirstCutOpen(true);
    if (action === "reviewProgram") {
      setDiagnosticView(reportForProgram ? "preflight" : "warnings");
      setDiagnosticsOpen(true);
    }
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
  const mockActions = senderActionLayout(displayedSender.state);
  const physicalActions = physicalSenderActionLayout(displayedSender.state);
  const mockStatus = displayedSenderFailure
    ? displayedSenderFailure
    : !dryRunAvailable
      ? "Подключите Mock GRBL в состоянии Idle"
      : displayedSender.state === "completed"
        ? "Все строки подтверждены Mock GRBL"
        : displayedSender.state === "cancelled"
          ? "Sender остановлен оператором"
          : "Каждая строка сопоставляется с ответом контроллера";

  const runMockPrimaryAction = () => {
    if (!dryRunGateway) return;
    if (mockActions.primary === "start") startDryRun();
    if (mockActions.primary === "pause") {
      void runSenderAction(dryRunGateway.pause);
    }
    if (mockActions.primary === "resume") {
      void runSenderAction(dryRunGateway.resume);
    }
  };

  useEffect(() => {
    if ((programRunVisible || checkRunVisible) && sender.currentSourceLine !== undefined) {
      setSelectedSourceLine(sender.currentSourceLine);
    }
  }, [checkRunVisible, programRunVisible, sender.currentSourceLine]);

  useEffect(() => {
    if (!checkRunVisible || sender.state !== "completed" || !realRunAvailable) return;
    setClearedSenderRunSequence(sender.runSequence);
    setRealRunReport(undefined);
    void runRealPreflight();
  }, [checkRunVisible, realRunAvailable, sender.runSequence, sender.state]);

  useEffect(() => {
    if (!recoveryCandidate || !firstCutOpen) return;
    setFirstCutOpen(false);
    setRecoveryOpen(true);
  }, [firstCutOpen, recoveryCandidate]);

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
                setClearedSenderRunSequence(sender.runSequence || undefined);
                setSender(idleSenderSnapshot);
                setSelectedSourceLine(undefined);
                setRealRunReport(undefined);
                setFirstCutOpen(false);
                setDiagnosticsOpen(false);
                setError(undefined);
              }}
              title="Закрыть программу"
              type="button"
            >
              <Trash2 aria-hidden="true" size={14} />
            </button>
          )}
          {program && (
            <ProgramFilePicker
              disabled={!desktopRuntime || senderActive}
              loading={loading}
              onSelect={(file) => void loadFile(file)}
              variant="toolbar"
            />
          )}
        </div>
      </header>

      {recoveryCandidate && !senderActive && (
        <aside
          className={`program-recovery-banner${recoveryCandidate.ready ? "" : " is-blocked"}`}
          role="status"
        >
          <History aria-hidden="true" size={18} />
          <div>
            <span>Нет подтверждения завершения</span>
            <strong>{recoveryCandidate.sourceName}</strong>
            <small>{recoveryCandidate.detail}</small>
          </div>
          <dl>
            <div>
              <dt>Executed</dt>
              <dd>{recoveryCandidate.executingSourceLine ?? "нет Ln"}</dd>
            </div>
            <div>
              <dt>Restart</dt>
              <dd>
                {recoveryCandidate.checkpointRestartAvailable
                  ? recoveryCandidate.restartSourceLine
                  : "полный"}
              </dd>
            </div>
          </dl>
          <button onClick={() => setRecoveryOpen(true)} type="button">
            Решить
          </button>
        </aside>
      )}

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
                  {programRunVisible &&
                    physicalActions.primary === "pause" &&
                    realRunGateway && (
                    <button
                      disabled={senderCommandBusy}
                      onClick={() =>
                        void runSenderAction(realRunGateway.pauseProgram)
                      }
                      type="button"
                    >
                      <Pause aria-hidden="true" size={13} />
                      Пауза
                    </button>
                  )}
                  {programRunVisible &&
                    physicalActions.primary === "resume" &&
                    realRunGateway && (
                    <button
                      disabled={senderCommandBusy}
                      onClick={() =>
                        void runSenderAction(realRunGateway.resumeProgram)
                      }
                      type="button"
                    >
                      <Play aria-hidden="true" size={13} />
                      Продолжить
                    </button>
                  )}
                  {programRunVisible &&
                    physicalActions.primary === "toolChange" &&
                    realRunGateway && (
                    <button
                      disabled={senderCommandBusy}
                      onClick={() => setToolChangeOpen(true)}
                      type="button"
                    >
                      <Wrench aria-hidden="true" size={13} />
                      Подтвердить замену
                    </button>
                  )}
                  {programRunVisible && physicalActions.stopVisible && realRunGateway && (
                    <button
                      aria-label={
                        stopConfirming
                          ? "Подтвердить завершение задания"
                          : "Завершить текущее задание"
                      }
                      className={`is-cancel${stopConfirming ? " is-confirming" : ""}`}
                      disabled={senderCommandBusy}
                      onClick={() => void stopProgramRun()}
                      title="Feed Hold, затем Soft Reset; незавершённую работу можно восстановить или закрыть"
                      type="button"
                    >
                      <Square aria-hidden="true" size={13} />
                      {stopConfirming ? "Ещё раз: завершить" : "Завершить задание"}
                    </button>
                  )}
                  {programRunVisible &&
                    physicalActions.primary === "prepareRerun" && (
                    <button
                      className="is-terminal-action"
                      onClick={() => {
                        setSender((current) => ({
                          ...idleSenderSnapshot,
                          runSequence: current.runSequence,
                        }));
                        setClearedSenderRunSequence(sender.runSequence);
                        setRealRunReport(undefined);
                      }}
                      type="button"
                    >
                      <RotateCcw aria-hidden="true" size={13} />
                      Подготовить повторный запуск
                    </button>
                  )}
                  {programRunVisible &&
                    physicalActions.primary === "resolveInterruption" && (
                    <button
                      className="is-terminal-action"
                      disabled={!recoveryChecked}
                      onClick={() => {
                        if (recoveryCandidate) {
                          setRecoveryOpen(true);
                        } else {
                          setClearedSenderRunSequence(sender.runSequence);
                          setSender((current) => ({
                            ...idleSenderSnapshot,
                            runSequence: current.runSequence,
                          }));
                        }
                      }}
                      type="button"
                    >
                      <History aria-hidden="true" size={13} />
                      {!recoveryChecked
                        ? "Сохраняем остановку..."
                        : recoveryCandidate
                          ? "Продолжить или начать заново"
                          : "Подготовить новый запуск"}
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
                      : displayedSender.state === "paused"
                        ? "Задание на паузе: продолжите его или завершите, чтобы освободить Jog"
                      : displayedSender.state === "cancelled"
                        ? "Задание завершено оператором; выберите восстановление или новый запуск"
                      : checkRunVisible
                        ? "По одной строке · без движения и включения выходов"
                        : "Пауза сохраняет продолжение; завершение останавливает поток через Hold и Reset"}
                </small>
              </div>
            ) : realRunTarget ? (
              <div className="job-readiness-shell">
                <JobReadinessPanel
                  busy={
                    preflightLoading ||
                    loading ||
                    senderActive ||
                    (machineContext?.busy ?? false)
                  }
                  details={{
                    machine: machineDetail,
                    file: fileDetail,
                    origin: originDetail,
                    validation: validationDetail,
                  }}
                  intent={programRunIntent}
                  onIntent={(intent) => {
                    setProgramRunIntent(intent);
                    setRealRunReport(undefined);
                  }}
                  onOpenOrigin={() => machineContext?.onOpenWorkZero()}
                  onPrimary={runReadinessAction}
                  view={readiness}
                />
                <details className="execution-settings">
                  <summary>
                    <SlidersHorizontal aria-hidden="true" size={13} />
                    <span>Дополнительные параметры</span>
                    <ChevronDown aria-hidden="true" size={13} />
                  </summary>
                  <div className="program-execution-options">
                    <label title="M1 Optional Stop">
                      <input
                        checked={programExecutionOptions.optionalStop}
                        disabled={preflightLoading || loading || senderActive}
                        onChange={(event) =>
                          void updateExecutionOption("optionalStop", event.target.checked)
                        }
                        type="checkbox"
                      />
                      <span>M1 Optional Stop</span>
                    </label>
                    <label title="Optional Block Delete">
                      <input
                        checked={programExecutionOptions.blockDelete}
                        disabled={preflightLoading || loading || senderActive}
                        onChange={(event) =>
                          void updateExecutionOption("blockDelete", event.target.checked)
                        }
                        type="checkbox"
                      />
                      <span>/ Block Delete</span>
                    </label>
                  </div>
                  <button
                    className="check-run-action"
                    disabled={!checkRunAvailable}
                    onClick={startCheckRun}
                    title="Проверить файл встроенным режимом GRBL $C без движения"
                    type="button"
                  >
                    <ScanSearch aria-hidden="true" size={13} />
                    Проверить через GRBL $C
                  </button>
                </details>
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
                  <button
                    aria-hidden={mockActions.primary === "none"}
                    className={mockActions.primary === "none" ? "is-placeholder" : undefined}
                    disabled={
                      !dryRunGateway ||
                      mockActions.primary === "none" ||
                      (mockActions.primary === "start" && !controls.canStart) ||
                      (mockActions.primary === "resume" && !controls.canResume)
                    }
                    onClick={runMockPrimaryAction}
                    tabIndex={mockActions.primary === "none" ? -1 : 0}
                    title={
                      mockActions.primary === "start" && !dryRunAvailable
                        ? "Подключите Mock GRBL в состоянии Idle"
                        : undefined
                    }
                    type="button"
                  >
                    {mockActions.primary === "pause" ? (
                      <Pause aria-hidden="true" size={13} />
                    ) : (
                      <Play aria-hidden="true" size={13} />
                    )}
                    {mockActions.primary === "pause"
                      ? "Pause"
                      : mockActions.primary === "resume"
                        ? "Resume"
                        : "Mock dry run"}
                  </button>
                  <button
                    aria-hidden={!mockActions.cancelVisible}
                    className={`is-cancel${mockActions.cancelVisible ? "" : " is-placeholder"}`}
                    disabled={!dryRunGateway || !mockActions.cancelVisible}
                    onClick={() => {
                      if (dryRunGateway && mockActions.cancelVisible) {
                        void runSenderAction(dryRunGateway.cancel);
                      }
                    }}
                    tabIndex={mockActions.cancelVisible ? 0 : -1}
                    type="button"
                  >
                    <X aria-hidden="true" size={13} />
                    Cancel
                  </button>
                </div>
                <small className={displayedSenderFailure ? "is-error" : undefined}>
                  {mockStatus}
                </small>
              </div>
            )}
            <details
              className="program-inspection"
              onToggle={(event) => setDiagnosticsOpen(event.currentTarget.open)}
              open={diagnosticsOpen}
            >
              <summary>
                <span>Программа и диагностика</span>
                <code>
                  {program.lines.length} строк
                  {program.warnings.length > 0
                    ? ` · ${program.warnings.length} предупреждений`
                    : ""}
                </code>
                <ChevronDown aria-hidden="true" size={13} />
              </summary>
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
            </details>
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
          <strong>Откройте программу для станка</strong>
          <span>Перетащите файл сюда или выберите его на диске</span>
          <ProgramFilePicker
            disabled={!desktopRuntime}
            loading={loading}
            onSelect={(file) => void loadFile(file)}
            variant="empty"
          />
          <code>.nc · .ngc · .gcode · .tap · .cnc</code>
        </div>
      )}

      <FirstCutAuthorizationDialog
        executionOptions={programExecutionOptions}
        intent={programRunIntent}
        onAuthorize={authorizeFirstCut}
        onAuthorized={(preparation) => {
          setRealRunReport(preparation.report);
          onInspection?.(preparation.report.hardware);
        }}
        onClose={() => setFirstCutOpen(false)}
        onStart={startProgramRun}
        onStarted={(snapshot) => {
          setClearedSenderRunSequence(undefined);
          setSender(snapshot);
          setDiagnosticsOpen(false);
          setRecoveryCandidate(undefined);
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

      {recoveryCandidate && realRunGateway && (
        <ProgramRecoveryDialog
          candidate={recoveryCandidate}
          onClose={() => setRecoveryOpen(false)}
          onDismiss={dismissRecovery}
          onPrepare={prepareRecovery}
          onPrepared={loadRecoveryPackage}
          open={recoveryOpen}
        />
      )}

      {error && <p className="program-error">{error}</p>}
    </section>
  );
}
