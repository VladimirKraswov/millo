import { useEffect, useMemo, useRef, useState, type DragEvent } from "react";
import { bindSnapshotStream } from "../../platform/state/bindSnapshotStream";
import { idleSenderSnapshot, type SenderSnapshot } from "../../shared/dryRun";
import type { JobToolAssignment } from "../../shared/jobs";
import type {
  FirstCutConfirmation,
  FirstCutPreparation,
  ProgramExecutionOptions,
  ProgramRunIntent,
  RunPreflightReport,
  SafeStartPackage,
  ToolChangeConfirmation,
} from "../../shared/realRun";
import { defaultProgramExecutionOptions } from "../../shared/realRun";
import type {
  ProgramRecoveryCandidate,
  ProgramRecoveryPackage,
  ProgramRecoveryPreparationRequest,
} from "../../shared/recovery";
import { type ProgramDiagnosticView } from "./ProgramInspection";
import { ProgramLoader, type LoadedProgram } from "./ProgramLoader";
import type { PreviewView } from "./ToolpathPreview";
import { canStartCheckRun } from "./checkRunReadModel";
import { depthAdjustmentUm, depthCorrectionView } from "./depthCorrectionModel";
import { senderFailureSummary } from "./dryRunReadModel";
import {
  executionOptionsForNewProgram,
  sameExecutionOptions,
} from "./executionOptionsModel";
import { type JobReadinessAction } from "./jobReadinessModel";
import {
  checkSenderAction,
  physicalSenderActionLayout,
  senderRunIsVisibleForProgram,
} from "./operatorLayoutModel";
import { hasActionableProgramWarnings } from "./programDiagnosticsModel";
import { programReadinessView } from "./programReadinessView";
import { programToolVisualization } from "./programToolVisualizationModel";
import type {
  ProgramWorkspaceProps,
  SafeStartContext,
} from "./programWorkspaceTypes";
import { isSenderActive } from "./senderStateModel";
import { useProgramSurface } from "./useProgramSurface";

export function useProgramWorkspace({
  desktopRuntime,
  gateway,
  heightmapGateway,
  initialProgram,
  initialRunIntent = "airRun",
  initialSender,
  initialSource = "",
  initialToolAssignments = [],
  incomingJob,
  machineContext,
  onInspection,
  onProgramChange,
  onError,
  realRunAvailable = false,
  realRunGateway,
  realRunTarget = false,
  senderGateway,
  tools = [],
}: ProgramWorkspaceProps) {
  const loader = useMemo(() => new ProgramLoader(gateway), [gateway]);
  const [loaded, setLoaded] = useState<LoadedProgram | undefined>(
    initialProgram
      ? { program: initialProgram, source: initialSource }
      : undefined,
  );
  useEffect(() => {
    onProgramChange?.(loaded?.program);
  }, [loaded?.program, onProgramChange]);
  const [sender, setSender] = useState<SenderSnapshot>(
    initialSender ?? idleSenderSnapshot,
  );
  const [toolAssignments, setToolAssignments] = useState<
    readonly JobToolAssignment[]
  >(initialToolAssignments);
  const [view, setView] = useState<PreviewView>("iso");
  const [loading, setLoading] = useState(false);
  const [dragging, setDragging] = useState(false);
  const [diagnosticView, setDiagnosticView] =
    useState<ProgramDiagnosticView>("lines");
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
  const [recoveryChecked, setRecoveryChecked] = useState(
    realRunGateway === undefined,
  );
  const [senderCommandBusy, setSenderCommandBusy] = useState(false);
  const [clearedSenderRunSequence, setClearedSenderRunSequence] =
    useState<number>();
  const [preflightLoading, setPreflightLoading] = useState(false);
  const [error, setError] = useState<string>();
  useEffect(() => {
    if (error) onError?.(error);
  }, [error, onError]);
  const handledIncomingJob = useRef(0);
  const reopenFirstCutAfterCheck = useRef(false);
  const program = loaded?.program;
  const senderActive = isSenderActive(sender.state);
  const {
    surfaceSession,
    surfaceMap,
    surfaceMapBusy,
    setSurfaceMapApplication,
  } = useProgramSurface({
    heightmapGateway,
    machineProfileId: machineContext?.machineProfileId,
    program,
    programExecutionOptions,
    setProgramExecutionOptions,
    setRealRunReport,
    setError,
    senderActive,
  });

  useEffect(() => {
    if (!incomingJob || incomingJob.sequence === handledIncomingJob.current)
      return;
    handledIncomingJob.current = incomingJob.sequence;
    if (senderActive) {
      setError(
        "Новое задание не открыто: сначала остановите текущее выполнение",
      );
      return;
    }
    setLoaded({
      program: incomingJob.job.program,
      source: incomingJob.job.source,
    });
    setToolAssignments(incomingJob.job.toolAssignments ?? []);
    setProgramRunIntent("cutting");
    const activeSurfaceMap = surfaceSession?.active;
    const activeSurfaceMapId =
      surfaceSession?.applicationEnabled &&
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
    return bindSnapshotStream({
      stream: {
        readCurrent: () => senderGateway.snapshot(),
        listen: (handler) => senderGateway.subscribe(handler),
      },
      onSnapshot: setSender,
      onError: (reason) => setError(String(reason)),
    });
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
      setToolAssignments([]);
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
      setToolAssignments([]);
      setProgramRunIntent(prepared.intent);
      const activeSurfaceMap = surfaceSession?.active;
      const restoredMapId =
        surfaceSession?.applicationEnabled &&
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
    setToolAssignments([]);
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
    () => program?.lines.find((line) => line.sourceLine === selectedSourceLine),
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
      await machineContext.onReturnToWorkOrigin(
        Math.max(2, program?.summary.bounds?.max.z ?? 0),
      );
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
  const toolVisualization = useMemo(
    () =>
      programToolVisualization(
        program ?? { lines: [] },
        displayedSender,
        programRunIntent,
        toolAssignments,
        tools,
      ),
    [displayedSender, program, programRunIntent, toolAssignments, tools],
  );
  const displayedSenderFailure = senderFailureSummary(displayedSender);
  const progressPercent = Math.round(
    Math.min(1, Math.max(0, displayedSender.progress)) * 100,
  );
  const depthCorrection = useMemo(
    () =>
      depthCorrectionView(
        program,
        programExecutionOptions.cuttingDepthAdjustmentUm,
      ),
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
    sameExecutionOptions(
      realRunReport.executionOptions,
      programExecutionOptions,
    )
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
  const {
    preflightControls,
    readiness,
    machineDetail,
    fileDetail,
    originDetail,
    validationDetail,
  } = programReadinessView({
    machineContext,
    realRunAvailable,
    realRunGateway,
    reportForProgram,
    preflightLoading,
    recoveryChecked,
    recoveryCandidate,
    program,
  });
  const runRealPreflight = async (): Promise<
    RunPreflightReport | undefined
  > => {
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
      return report;
    } catch (reason) {
      setError(String(reason));
      return undefined;
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
  const applySurfaceMapFromFirstCut = async (
    enabled: boolean,
  ): Promise<void> => {
    if (!loaded || !realRunGateway) {
      throw new Error("GRBL Check недоступен для текущего задания");
    }
    const executionOptions = await setSurfaceMapApplication(enabled);
    setSenderCommandBusy(true);
    setError(undefined);
    reopenFirstCutAfterCheck.current = true;
    try {
      const checkSnapshot = await realRunGateway.startCheck(
        {
          sourceName: loaded.program.sourceName,
          source: loaded.source,
        },
        executionOptions,
      );
      setClearedSenderRunSequence(undefined);
      setSender(checkSnapshot);
      setDiagnosticsOpen(false);
      setFirstCutOpen(false);
    } catch (reason) {
      reopenFirstCutAfterCheck.current = false;
      setError(String(reason));
      throw reason;
    } finally {
      setSenderCommandBusy(false);
    }
  };
  const runReadinessAction = (action: JobReadinessAction) => {
    if (action === "connect") void machineContext?.onConnect();
    if (action === "unlock") void machineContext?.onUnlock();
    if (action === "acknowledgeReset")
      void machineContext?.onAcknowledgeReset();
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
      (check) =>
        check.id === "grbl-check-certificate" && check.level === "pass",
    ) ?? false;
  const safeStartCheckLabel = checkRunVisible
    ? displayedSender.state === "completed"
      ? "GRBL Check завершён"
      : displayedSender.state === "failed" ||
          displayedSender.state === "cancelled"
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
    if (
      (programRunVisible || checkRunVisible) &&
      sender.currentSourceLine !== undefined
    ) {
      setSelectedSourceLine(sender.currentSourceLine);
    }
  }, [checkRunVisible, programRunVisible, sender.currentSourceLine]);

  useEffect(() => {
    if (!checkRunVisible) return;
    if (sender.state === "failed" || sender.state === "cancelled") {
      reopenFirstCutAfterCheck.current = false;
      return;
    }
    if (sender.state !== "completed" || !realRunAvailable) return;
    setClearedSenderRunSequence(sender.runSequence);
    setRealRunReport(undefined);
    void runRealPreflight().then((report) => {
      if (!reopenFirstCutAfterCheck.current) return;
      reopenFirstCutAfterCheck.current = false;
      if (report?.ready) setFirstCutOpen(true);
    });
  }, [checkRunVisible, realRunAvailable, sender.runSequence, sender.state]);

  useEffect(() => {
    if (!recoveryCandidate || !firstCutOpen) return;
    setFirstCutOpen(false);
    setRecoveryOpen(true);
  }, [firstCutOpen, recoveryCandidate]);
  return {
    applyEditedProgram,
    applySurfaceMapFromFirstCut,
    authorizeFirstCut,
    bounds,
    checkAction,
    checkRunAvailable,
    checkRunVisible,
    completeToolChange,
    depthCorrection,
    desktopRuntime,
    diagnosticView,
    diagnosticsOpen,
    dismissRecovery,
    displayedSender,
    displayedSenderFailure,
    dragging,
    dropFile,
    editorOpen,
    error,
    fileDetail,
    firstCutOpen,
    gateway,
    loadFile,
    loadRecoveryPackage,
    loadSafeStartPackage,
    loaded,
    loading,
    machineContext,
    machineDetail,
    motionSourceLines,
    onError,
    onInspection,
    originDetail,
    physicalActions,
    preflightLoading,
    prepareRecovery,
    prepareSelectedRun,
    program,
    programExecutionOptions,
    programRunIntent,
    programRunVisible,
    progressPercent,
    readiness,
    realRunAvailable,
    realRunGateway,
    realRunTarget,
    recoveryCandidate,
    recoveryChecked,
    recoveryOpen,
    reportForProgram,
    restoreFullProgram,
    returnFromCheck,
    returnToWorkOrigin,
    runReadinessAction,
    runSenderAction,
    safeStartCheckLabel,
    safeStartCheckPassed,
    safeStartContext,
    safeStartOpen,
    selectedMotionCount,
    selectedProgramLine,
    selectedSourceLine,
    sender,
    senderActive,
    senderCommandBusy,
    setClearedSenderRunSequence,
    setDepthAdjustment,
    setDepthCorrectionEnabled,
    setDiagnosticView,
    setDiagnosticsOpen,
    setDragging,
    setEditorOpen,
    setError,
    setFirstCutOpen,
    setLoaded,
    setProgramExecutionOptions,
    setProgramRunIntent,
    setRealRunReport,
    setRecoveryCandidate,
    setRecoveryOpen,
    setSafeStartContext,
    setSafeStartOpen,
    setSelectedSourceLine,
    setSender,
    setSurfaceMapApplication,
    setToolAssignments,
    setToolChangeOpen,
    setView,
    startCheckRun,
    startProgramRun,
    stopProgramRun,
    surfaceMap,
    surfaceMapBusy,
    toolChangeOpen,
    toolVisualization,
    updateExecutionOption,
    validationDetail,
    view,
  };
}
