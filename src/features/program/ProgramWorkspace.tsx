import {
  Box,
  ChevronDown,
  FileCode2,
  History,
  PencilLine,
  RotateCcw,
  Route,
  ScanSearch,
  ShieldAlert,
  SlidersHorizontal,
  Square,
  Trash2,
  X,
} from "lucide-react";
import {
  useEffect,
  useMemo,
  useRef,
  useState,
  type DragEvent,
} from "react";

import type { ProgramGateway } from "../../platform/program/ProgramGateway";
import type { HeightmapGateway } from "../../platform/machine/HeightmapGateway";
import type {
  ControllerSnapshot,
  HardwareInspection,
  Position,
} from "../../shared/machine";
import {
  idleSenderSnapshot,
  type SenderSnapshot,
  type SenderStateGateway,
} from "../../shared/dryRun";
import type { GcodeProgram } from "../../shared/program";
import type { PublishedJob } from "../../shared/jobs";
import type { SurfaceSession } from "../../shared/heightmap";
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
  SafeStartPackage,
  ToolChangeConfirmation,
} from "../../shared/realRun";
import { FirstCutAuthorizationDialog } from "./FirstCutAuthorizationDialog";
import { JobReadinessPanel } from "./JobReadinessPanel";
import { ProgramFilePicker } from "./ProgramFilePicker";
import { ProgramEditor } from "./ProgramEditor";
import { ProgramInspection, type ProgramDiagnosticView } from "./ProgramInspection";
import { ProgramLoader, type LoadedProgram } from "./ProgramLoader";
import { ProgramPreviewStage } from "./ProgramPreviewStage";
import { ProgramRecoveryDialog } from "./ProgramRecoveryDialog";
import { ProgramRunCard } from "./ProgramRunCard";
import { SafeStartDialog } from "./SafeStartDialog";
import { ToolChangeDialog } from "./ToolChangeDialog";
import { canStartCheckRun } from "./checkRunReadModel";
import { senderFailureSummary } from "./dryRunReadModel";
import { realRunPreflightControls } from "./realRunPreflightReadModel";
import {
  checkSenderAction,
  physicalSenderActionLayout,
  senderRunIsVisibleForProgram,
} from "./operatorLayoutModel";
import {
  jobReadinessModel,
  type JobReadinessAction,
} from "./jobReadinessModel";
import type { PreviewView } from "./ToolpathPreview";
import { suggestedSafeZ } from "./safeStartModel";
import { surfaceMapExecutionView } from "./surfaceMapExecutionModel";
import {
  depthAdjustmentUm,
  depthCorrectionView,
} from "./depthCorrectionModel";
import {
  executionOptionsForNewProgram,
  sameExecutionOptions,
} from "./executionOptionsModel";
import { isSenderActive } from "./senderStateModel";
import { initialProgramToolNumber } from "./programToolPlanModel";
import {
  formatProgramDiagnostics,
  hasActionableProgramWarnings,
  programCanEnterPreflight,
  programDiagnosticsSummary,
} from "./programDiagnosticsModel";

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

interface ProgramWorkspaceProps {
  readonly desktopRuntime: boolean;
  readonly gateway: ProgramGateway;
  readonly heightmapGateway?: HeightmapGateway;
  readonly initialProgram?: GcodeProgram;
  readonly initialRunIntent?: ProgramRunIntent;
  readonly initialSender?: SenderSnapshot;
  readonly initialSource?: string;
  readonly incomingJob?: PublishedJob;
  readonly machineContext?: ProgramMachineContext;
  readonly onInspection?: (inspection: HardwareInspection) => void;
  readonly onProgramChange?: (program?: GcodeProgram) => void;
  readonly realRunAvailable?: boolean;
  readonly realRunGateway?: RealRunPreflightGateway;
  readonly realRunTarget?: boolean;
  readonly senderGateway?: SenderStateGateway;
}

interface SafeStartContext {
  readonly original: LoadedProgram;
  readonly package: SafeStartPackage;
}

export function ProgramWorkspace({
  desktopRuntime,
  gateway,
  heightmapGateway,
  initialProgram,
  initialRunIntent = "airRun",
  initialSender,
  initialSource = "",
  incomingJob,
  machineContext,
  onInspection,
  onProgramChange,
  realRunAvailable = false,
  realRunGateway,
  realRunTarget = false,
  senderGateway,
}: ProgramWorkspaceProps) {
  const loader = useMemo(() => new ProgramLoader(gateway), [gateway]);
  const [loaded, setLoaded] = useState<LoadedProgram | undefined>(
    initialProgram ? { program: initialProgram, source: initialSource } : undefined,
  );
  useEffect(() => {
    onProgramChange?.(loaded?.program);
  }, [loaded?.program, onProgramChange]);
  const [sender, setSender] = useState<SenderSnapshot>(
    initialSender ?? idleSenderSnapshot,
  );
  const [view, setView] = useState<PreviewView>("iso");
  const [loading, setLoading] = useState(false);
  const [dragging, setDragging] = useState(false);
  const [diagnosticView, setDiagnosticView] = useState<ProgramDiagnosticView>("lines");
  const [diagnosticsOpen, setDiagnosticsOpen] = useState(
    initialProgram ? hasActionableProgramWarnings(initialProgram) : false,
  );
  const [selectedSourceLine, setSelectedSourceLine] = useState<number>();
  const [realRunReport, setRealRunReport] = useState<RunPreflightReport>();
  const [programRunIntent, setProgramRunIntent] =
    useState<ProgramRunIntent>(initialRunIntent);
  const [programExecutionOptions, setProgramExecutionOptions] =
    useState<ProgramExecutionOptions>(defaultProgramExecutionOptions);
  const [firstCutOpen, setFirstCutOpen] = useState(false);
  const [safeStartOpen, setSafeStartOpen] = useState(false);
  const [editorOpen, setEditorOpen] = useState(false);
  const [safeStartContext, setSafeStartContext] = useState<SafeStartContext>();
  const [toolChangeOpen, setToolChangeOpen] = useState(false);
  const [recoveryOpen, setRecoveryOpen] = useState(false);
  const [recoveryCandidate, setRecoveryCandidate] =
    useState<ProgramRecoveryCandidate>();
  const [recoveryChecked, setRecoveryChecked] = useState(realRunGateway === undefined);
  const [senderCommandBusy, setSenderCommandBusy] = useState(false);
  const [clearedSenderRunSequence, setClearedSenderRunSequence] = useState<number>();
  const [preflightLoading, setPreflightLoading] = useState(false);
  const [surfaceSession, setSurfaceSession] = useState<SurfaceSession>();
  const [surfaceMapBusy, setSurfaceMapBusy] = useState(false);
  const [error, setError] = useState<string>();
  const handledIncomingJob = useRef(0);
  const program = loaded?.program;
  const senderActive = isSenderActive(sender.state);

  useEffect(() => {
    if (!heightmapGateway) return;
    let active = true;
    let unsubscribe: (() => void) | undefined;
    const accept = (session: SurfaceSession) => {
      if (!active) return;
      setSurfaceSession(session);
      setProgramExecutionOptions((current) => {
        const usable = session.applicationEnabled &&
          session.coordinateBindingStale === false &&
          session.active?.machineProfileId === machineContext?.machineProfileId;
        const surfaceMapId = usable ? session.active?.mapId : undefined;
        return current.surfaceMapId === surfaceMapId
          ? current
          : { ...current, surfaceMapId };
      });
      setRealRunReport(undefined);
    };
    void heightmapGateway.getSession().then(accept).catch((reason: unknown) => {
      if (active) setError(String(reason));
    });
    void heightmapGateway.subscribeSession(accept).then((stop) => {
      if (active) unsubscribe = stop;
      else stop();
    }).catch((reason: unknown) => {
      if (active) setError(String(reason));
    });
    return () => {
      active = false;
      unsubscribe?.();
    };
  }, [heightmapGateway, machineContext?.machineProfileId]);

  useEffect(() => {
    if (!incomingJob || incomingJob.sequence === handledIncomingJob.current) return;
    handledIncomingJob.current = incomingJob.sequence;
    if (senderActive) {
      setError("Новое задание не открыто: сначала остановите текущее выполнение");
      return;
    }
    setLoaded({ program: incomingJob.job.program, source: incomingJob.job.source });
    setProgramRunIntent("cutting");
    const activeSurfaceMap = surfaceSession?.active;
    const activeSurfaceMapId = surfaceSession?.applicationEnabled &&
      activeSurfaceMap &&
      activeSurfaceMap?.machineProfileId === machineContext?.machineProfileId
      ? activeSurfaceMap.mapId
      : undefined;
    setProgramExecutionOptions({
      ...defaultProgramExecutionOptions,
      surfaceMapId: activeSurfaceMapId,
    });
    setClearedSenderRunSequence(sender.runSequence || undefined);
    setSender(idleSenderSnapshot);
    setSelectedSourceLine(undefined);
    const hasWarnings = hasActionableProgramWarnings(incomingJob.job.program);
    setDiagnosticView(hasWarnings ? "warnings" : "lines");
    setDiagnosticsOpen(hasWarnings);
    setRealRunReport(undefined);
    setFirstCutOpen(false);
    setSafeStartOpen(false);
    setEditorOpen(false);
    setSafeStartContext(undefined);
    setToolChangeOpen(false);
    setError(undefined);
  }, [
    incomingJob,
    machineContext?.machineProfileId,
    sender.runSequence,
    senderActive,
    surfaceSession?.active?.machineProfileId,
    surfaceSession?.active?.mapId,
    surfaceSession?.applicationEnabled,
  ]);

  useEffect(() => {
    if (!desktopRuntime || !senderGateway) return;
    let active = true;
    let unsubscribe: (() => void) | undefined;
    void senderGateway
      .snapshot()
      .then((snapshot) => {
        if (active) setSender(snapshot);
      })
      .catch((reason: unknown) => {
        if (active) setError(String(reason));
      });
    void senderGateway
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
  }, [desktopRuntime, senderGateway]);

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
      (sender.state !== "failed" && sender.state !== "cancelled") ||
      (sender.mode !== "airRun" && sender.mode !== "cutRun")
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
  }, [realRunGateway, sender.mode, sender.runSequence, sender.state]);

  useEffect(() => {
    if (!realRunTarget || !realRunAvailable) {
      setRealRunReport(undefined);
      setFirstCutOpen(false);
      setSafeStartOpen(false);
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
      const next = await loader.load(file);
      setLoaded(next);
      setProgramExecutionOptions(executionOptionsForNewProgram);
      setClearedSenderRunSequence(sender.runSequence || undefined);
      setSender(idleSenderSnapshot);
      setSelectedSourceLine(undefined);
      const hasWarnings = hasActionableProgramWarnings(next.program);
      setDiagnosticView(hasWarnings ? "warnings" : "lines");
      setDiagnosticsOpen(hasWarnings);
      setRealRunReport(undefined);
      setFirstCutOpen(false);
      setToolChangeOpen(false);
      setEditorOpen(false);
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
      const activeSurfaceMap = surfaceSession?.active;
      const restoredMapId = surfaceSession?.applicationEnabled &&
        activeSurfaceMap &&
        activeSurfaceMap?.mapId === prepared.executionOptions.surfaceMapId &&
        activeSurfaceMap.machineProfileId === machineContext?.machineProfileId
        ? prepared.executionOptions.surfaceMapId
        : undefined;
      setProgramExecutionOptions({
        ...prepared.executionOptions,
        surfaceMapId: restoredMapId,
      });
      setClearedSenderRunSequence(sender.runSequence || undefined);
      setSender(idleSenderSnapshot);
      setSelectedSourceLine(prepared.restartSourceLine);
      setDiagnosticView("lines");
      setDiagnosticsOpen(false);
      setRealRunReport(undefined);
      setFirstCutOpen(false);
      setSafeStartOpen(false);
      setSafeStartContext(undefined);
      setEditorOpen(false);
      setRecoveryCandidate(undefined);
    } catch (reason) {
      setError(String(reason));
      throw reason;
    } finally {
      setLoading(false);
    }
  };

  const prepareSelectedRun = (safeZMm: number): Promise<SafeStartPackage> => {
    if (!loaded || !realRunGateway || selectedSourceLine === undefined) {
      throw new Error("Safe selected-line start is unavailable");
    }
    return realRunGateway.prepareSelectedRun({
      request: {
        sourceName: loaded.program.sourceName,
        source: loaded.source,
      },
      selectedSourceLine,
      safeZMm,
      intent: programRunIntent,
      executionOptions: programExecutionOptions,
    });
  };

  const loadSafeStartPackage = async (prepared: SafeStartPackage) => {
    if (!loaded || !realRunGateway) return;
    setLoading(true);
    setError(undefined);
    try {
      const nextProgram = await gateway.parse(prepared.request, {
        blockDelete: programExecutionOptions.blockDelete,
      });
      const checkSnapshot = await realRunGateway.startCheck(
        prepared.request,
        programExecutionOptions,
      );
      const original = safeStartContext?.original ?? loaded;
      setLoaded({ program: nextProgram, source: prepared.request.source });
      setSafeStartContext({ original, package: prepared });
      setClearedSenderRunSequence(undefined);
      setSender(checkSnapshot);
      setSelectedSourceLine(undefined);
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

  const restoreFullProgram = () => {
    if (!safeStartContext || senderActive) return;
    setLoaded(safeStartContext.original);
    setSafeStartContext(undefined);
    setSafeStartOpen(false);
    setClearedSenderRunSequence(sender.runSequence || undefined);
    setSender(idleSenderSnapshot);
    setSelectedSourceLine(undefined);
    setRealRunReport(undefined);
    setDiagnosticsOpen(false);
    setError(undefined);
  };

  const applyEditedProgram = (next: LoadedProgram) => {
    setLoaded(next);
    setClearedSenderRunSequence(sender.runSequence || undefined);
    setSender(idleSenderSnapshot);
    setSelectedSourceLine(undefined);
    const hasWarnings = hasActionableProgramWarnings(next.program);
    setDiagnosticView(hasWarnings ? "warnings" : "lines");
    setDiagnosticsOpen(hasWarnings);
    setRealRunReport(undefined);
    setFirstCutOpen(false);
    setSafeStartOpen(false);
    setSafeStartContext(undefined);
    setToolChangeOpen(false);
    setEditorOpen(false);
    setError(undefined);
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

  const returnToWorkOrigin = async () => {
    if (!machineContext || senderCommandBusy) return;
    setSenderCommandBusy(true);
    setError(undefined);
    try {
      await machineContext.onReturnToWorkOrigin(Math.max(2, program?.summary.bounds?.max.z ?? 0));
    } catch (reason) {
      setError(String(reason));
    } finally {
      setSenderCommandBusy(false);
    }
  };

  const stopProgramRun = async () => {
    if (!realRunGateway || senderCommandBusy) return;
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
      setSenderCommandBusy(false);
    }
  };
  const senderForProgram = senderRunIsVisibleForProgram(
    sender,
    program?.sourceName,
    clearedSenderRunSequence,
  );
  const displayedSender = senderForProgram ? sender : idleSenderSnapshot;
  const displayedSenderFailure = senderFailureSummary(displayedSender);
  const progressPercent = Math.round(
    Math.min(1, Math.max(0, displayedSender.progress)) * 100,
  );
  const depthCorrection = useMemo(
    () => depthCorrectionView(program, programExecutionOptions.cuttingDepthAdjustmentUm),
    [program, programExecutionOptions.cuttingDepthAdjustmentUm],
  );
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
    sameExecutionOptions(realRunReport.executionOptions, programExecutionOptions)
      ? realRunReport
      : undefined;
  const setDepthCorrectionEnabled = (enabled: boolean) => {
    if (!depthCorrection.available) return;
    setRealRunReport(undefined);
    setProgramExecutionOptions((current) => ({
      ...current,
      cuttingDepthAdjustmentUm: enabled ? 0 : undefined,
    }));
  };
  const setDepthAdjustment = (adjustmentMm: number) => {
    if (!Number.isFinite(adjustmentMm)) return;
    try {
      const cuttingDepthAdjustmentUm = depthAdjustmentUm(adjustmentMm);
      setError(undefined);
      setRealRunReport(undefined);
      setProgramExecutionOptions((current) => ({
        ...current,
        cuttingDepthAdjustmentUm,
      }));
    } catch (reason) {
      setError(String(reason));
    }
  };
  const surfaceMap = useMemo(
    () => surfaceMapExecutionView(
      surfaceSession,
      machineContext?.machineProfileId,
      program?.summary.bounds,
    ),
    [machineContext?.machineProfileId, program?.summary.bounds, surfaceSession],
  );
  const setSurfaceMapApplication = async (enabled: boolean) => {
    if (!heightmapGateway || !surfaceMap || surfaceMapBusy || senderActive) return;
    if (enabled && !surfaceMap.coversProgram) {
      setError("Карта высот не покрывает траекторию задания. Снимите карту по периметру файла.");
      return;
    }
    if (enabled && !surfaceMap.usable) {
      setError("Рабочий ноль изменился после измерения карты. Сначала снимите новую карту высот.");
      return;
    }
    setSurfaceMapBusy(true);
    setError(undefined);
    setRealRunReport(undefined);
    try {
      const session = await heightmapGateway.setApplication(enabled, enabled);
      setSurfaceSession(session);
      setProgramExecutionOptions((current) => ({
        ...current,
        surfaceMapId: enabled ? session.active?.mapId : undefined,
      }));
    } catch (reason) {
      setError(String(reason));
    } finally {
      setSurfaceMapBusy(false);
    }
  };
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
    machineSyncing: machineContext?.machineSyncing ?? false,
    machineMode: machineContext?.snapshot.machine.mode ?? "unknown",
    parserEligible: program ? programCanEnterPreflight(program) : false,
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
          : machineContext.machineSyncing
            ? "Читаем привязку профиля из контроллера"
            : !machineContext.machineBound
              ? "Профиль не синхронизирован с подключённым контроллером"
          : `${machineContext.machineName} · ${machineContext.snapshot.machine.reportedMode}`
    : "Не подключен";
  const programDiagnostics = program ? programDiagnosticsSummary(program) : undefined;
  const programDiagnosticsDetail = programDiagnostics
    ? formatProgramDiagnostics(programDiagnostics)
    : "";
  const fileDetail = program && programCanEnterPreflight(program)
    ? `${program.summary.lineCount} строк${programDiagnosticsDetail ? ` · ${programDiagnosticsDetail}` : ""}`
    : programDiagnostics?.actionableCount
      ? `${programDiagnostics.actionableCount} замечаний требуют внимания`
      : program
        ? "Не удалось построить полный preview"
        : "Файл не загружен";
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
    if (action === "syncMachine") void machineContext?.onSyncMachine();
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
  const safeStartCheckPassed =
    reportForProgram?.checks.some(
      (check) => check.id === "grbl-check-certificate" && check.level === "pass",
    ) ?? false;
  const safeStartCheckLabel = checkRunVisible
    ? displayedSender.state === "completed"
      ? "GRBL Check завершён"
      : displayedSender.state === "failed" || displayedSender.state === "cancelled"
        ? "GRBL Check не пройден"
      : "GRBL Check идёт"
    : safeStartCheckPassed
      ? "GRBL Check пройден"
      : "Требуется GRBL Check";
  const checkRunAvailable = canStartCheckRun(displayedSender, {
    gatewayAvailable: realRunGateway !== undefined,
    loading,
    programLoaded: loaded !== undefined,
    serialAvailable: realRunAvailable,
  });
  const physicalActions = physicalSenderActionLayout(displayedSender.state);
  const checkAction = checkSenderAction(displayedSender.state);

  const returnFromCheck = () => {
    setClearedSenderRunSequence(sender.runSequence);
    setSender((current) => ({
      ...idleSenderSnapshot,
      runSequence: current.runSequence,
    }));
    setRealRunReport(undefined);
    setDiagnosticsOpen(displayedSender.state === "failed");
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
          <span>Программа</span>
          <strong id="program-title">{program?.sourceName ?? "Предпросмотр G-code"}</strong>
        </div>
        <div className="program-actions">
          {program && (
            <div className="preview-view" role="group" aria-label="Вид траектории">
              <button
                aria-label="Вид сверху"
                aria-pressed={view === "top"}
                onClick={() => setView("top")}
                title="Вид сверху"
                type="button"
              >
                <Square aria-hidden="true" size={14} />
              </button>
              <button
                aria-label="Изометрический вид"
                aria-pressed={view === "iso"}
                onClick={() => setView("iso")}
                title="Изометрический вид"
                type="button"
              >
                <Box aria-hidden="true" size={14} />
              </button>
            </div>
          )}
          {program && (
            <button
              aria-label="Редактировать G-code"
              className="program-icon-action"
              disabled={senderActive || safeStartContext !== undefined}
              onClick={() => setEditorOpen(true)}
              title={
                safeStartContext
                  ? "Вернитесь к полной программе перед редактированием"
                  : "Редактировать G-code"
              }
              type="button"
            >
              <PencilLine aria-hidden="true" size={14} />
            </button>
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
                setSafeStartOpen(false);
                setSafeStartContext(undefined);
                setEditorOpen(false);
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
              <dt>Выполнено</dt>
              <dd>{recoveryCandidate.executingSourceLine ?? "нет Ln"}</dd>
            </div>
            <div>
              <dt>Начать с</dt>
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

      {safeStartContext && program && (
        <aside className="safe-start-banner" role="status">
          <Route aria-hidden="true" size={18} />
          <div>
            <span>Безопасный частичный запуск</span>
            <strong>
              Выбрано L{safeStartContext.package.selectedSourceLine} · вход с L
              {safeStartContext.package.restartSourceLine}
            </strong>
            <small>
              Safe Z {safeStartContext.package.safeZMm.toFixed(3)} mm · {safeStartContext.package.workCoordinateSystem.toUpperCase()}
              {safeStartContext.package.selectedTool === undefined
                ? ""
                : ` · T${safeStartContext.package.selectedTool}`}
              {safeStartContext.package.replayedExecutableLines > 0
                ? ` · повтор ${safeStartContext.package.replayedExecutableLines} строк до выбранной`
                : " · вход точно с выбранной строки"}
            </small>
          </div>
          <span
            className={`safe-start-check-badge${safeStartCheckPassed ? " is-pass" : ""}`}
          >
            <ScanSearch aria-hidden="true" size={13} />
            {safeStartCheckLabel}
          </span>
          <button
            disabled={senderActive}
            onClick={restoreFullProgram}
            type="button"
          >
            <RotateCcw aria-hidden="true" size={13} />
            Вся программа
          </button>
        </aside>
      )}

      {program ? (
        <div className="program-body">
          <ProgramPreviewStage
            cuttingDepthAdjustmentMm={depthCorrection.enabled
              ? depthCorrection.adjustmentMm
              : 0}
            onClearSelection={() => setSelectedSourceLine(undefined)}
            onSafeStart={() => setSafeStartOpen(true)}
            onSelectSourceLine={setSelectedSourceLine}
            program={program}
            safeStartAvailable={Boolean(
              selectedProgramLine &&
              realRunTarget &&
              realRunGateway &&
              realRunAvailable &&
              selectedMotionCount > 0 &&
              !recoveryCandidate &&
              !senderActive,
            )}
            selectedMotionCount={selectedMotionCount}
            selectedProgramLine={selectedProgramLine}
            selectedSourceLine={selectedSourceLine}
            toolCoordinateSystem={machineContext?.activeCoordinateSystem}
            toolPosition={machineContext?.workPosition}
            view={view}
          />

          <aside className="program-diagnostics" aria-label="Выполнение и диагностика программы">
            {realRunTarget && (programRunVisible || checkRunVisible) ? (
              <ProgramRunCard
                busy={senderCommandBusy}
                checkAction={checkAction}
                checkControlsAvailable={realRunGateway !== undefined}
                checkRun={checkRunVisible}
                failureSummary={displayedSenderFailure}
                machineContextAvailable={machineContext !== undefined}
                onCancelCheck={() => {
                  if (realRunGateway) void runSenderAction(realRunGateway.abortProgram);
                }}
                onPause={() => {
                  if (realRunGateway) void runSenderAction(realRunGateway.pauseProgram);
                }}
                onPrepareRerun={() => {
                  setSender((current) => ({
                    ...idleSenderSnapshot,
                    runSequence: current.runSequence,
                  }));
                  setClearedSenderRunSequence(sender.runSequence);
                  setRealRunReport(undefined);
                }}
                onResolveInterruption={() => {
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
                onResume={() => {
                  if (realRunGateway) void runSenderAction(realRunGateway.resumeProgram);
                }}
                onReturnFromCheck={returnFromCheck}
                onReturnToWorkOrigin={() => void returnToWorkOrigin()}
                onStop={() => void stopProgramRun()}
                onToolChange={() => setToolChangeOpen(true)}
                physicalActions={physicalActions}
                programControlsAvailable={realRunGateway !== undefined}
                programRun={programRunVisible}
                progressPercent={progressPercent}
                recoveryAvailable={recoveryCandidate !== undefined}
                recoveryChecked={recoveryChecked}
                sender={displayedSender}
              />
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
                  intentLocked={safeStartContext !== undefined}
                  onIntent={(intent) => {
                    setProgramRunIntent(intent);
                    if (intent === "airRun") {
                      setProgramExecutionOptions((current) => ({
                        ...current,
                        cuttingDepthAdjustmentUm: undefined,
                      }));
                    }
                    setRealRunReport(undefined);
                  }}
                  onOpenOrigin={() => machineContext?.onOpenWorkZero()}
                  depthCorrection={depthCorrection}
                  onDepthCorrectionEnabled={setDepthCorrectionEnabled}
                  onDepthAdjustment={setDepthAdjustment}
                  onPrimary={runReadinessAction}
                  onSurfaceMap={(enabled) => void setSurfaceMapApplication(enabled)}
                  surfaceMap={surfaceMap ? {
                    checked: surfaceMap.enabled &&
                      programExecutionOptions.surfaceMapId === surfaceMap.map.mapId,
                    detail: surfaceMap.detail,
                    disabled: surfaceMapBusy || senderActive || !surfaceMap.coversProgram || !surfaceMap.usable,
                    warning: !surfaceMap.coversProgram || !surfaceMap.usable,
                  } : undefined}
                  view={readiness}
                />
                <details className="execution-settings">
                  <summary>
                    <SlidersHorizontal aria-hidden="true" size={13} />
                    <span>Дополнительные параметры</span>
                    <ChevronDown aria-hidden="true" size={13} />
                  </summary>
                  <div className="program-execution-options">
                    <label title="Остановка программы по M1">
                      <input
                        checked={programExecutionOptions.optionalStop}
                        disabled={preflightLoading || loading || senderActive}
                        onChange={(event) =>
                          void updateExecutionOption("optionalStop", event.target.checked)
                        }
                        type="checkbox"
                      />
                      <span>Остановка по M1</span>
                    </label>
                    <label title="Не выполнять строки, начинающиеся с /">
                      <input
                        checked={programExecutionOptions.blockDelete}
                        disabled={preflightLoading || loading || senderActive}
                        onChange={(event) =>
                          void updateExecutionOption("blockDelete", event.target.checked)
                        }
                        type="checkbox"
                      />
                      <span>Пропуск строк с /</span>
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
            ) : null}
            <ProgramInspection
              diagnosticView={diagnosticView}
              motionSourceLines={motionSourceLines}
              onOpenChange={setDiagnosticsOpen}
              onSelectSourceLine={setSelectedSourceLine}
              onView={setDiagnosticView}
              open={diagnosticsOpen}
              program={program}
              realRunTarget={realRunTarget}
              report={reportForProgram}
              selectedSourceLine={selectedSourceLine}
            />
          </aside>
        </div>
      ) : senderActive && realRunGateway ? (
        <div className="program-dropzone sender-recovery" role="status">
          <ShieldAlert aria-hidden="true" size={28} />
          <strong>{sender.sourceName ?? "Проверка движения"}</strong>
          <span>Выполнение активно</span>
          <button
            onClick={() => void runSenderAction(realRunGateway.abortProgram)}
            type="button"
          >
            <X aria-hidden="true" size={13} />
            Остановить выполнение
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

      {selectedProgramLine && (
        <SafeStartDialog
          minimumSafeZ={bounds?.max.z ?? 0}
          motionCount={selectedMotionCount}
          onClose={() => setSafeStartOpen(false)}
          onPrepare={prepareSelectedRun}
          onPrepared={loadSafeStartPackage}
          open={safeStartOpen}
          selectedCommand={selectedProgramLine.source || selectedProgramLine.normalized}
          sourceLine={selectedProgramLine.sourceLine}
          suggestedSafeZ={suggestedSafeZ(bounds?.max.z)}
        />
      )}

      {editorOpen && loaded && (
        <ProgramEditor
          blockDelete={programExecutionOptions.blockDelete}
          document={loaded}
          gateway={gateway}
          onApply={applyEditedProgram}
          onClose={() => setEditorOpen(false)}
        />
      )}

      <FirstCutAuthorizationDialog
        depthCorrection={
          depthCorrection.enabled
            ? { adjustmentMm: depthCorrection.adjustmentMm }
            : undefined
        }
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
        startingToolNumber={program ? initialProgramToolNumber(program) : undefined}
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

      {error && (
        <div className="program-error" role="alert">
          <span>{error}</span>
          <button aria-label="Закрыть сообщение" onClick={() => setError(undefined)} type="button">×</button>
        </div>
      )}
    </section>
  );
}
