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
import { lazy, Suspense } from "react";
import { idleSenderSnapshot } from "../../shared/dryRun";
import { JobReadinessPanel } from "./JobReadinessPanel";
import { ProgramEditor } from "./ProgramEditor";
import { ProgramFilePicker } from "./ProgramFilePicker";
import { ProgramInspection } from "./ProgramInspection";
import { ProgramPreviewStage } from "./ProgramPreviewStage";
import { ProgramRecoveryDialog } from "./ProgramRecoveryDialog";
import { ProgramRunCard } from "./ProgramRunCard";
import { SafeStartDialog } from "./SafeStartDialog";
import { ToolChangeDialog } from "./ToolChangeDialog";
import { initialProgramToolNumber } from "./programToolPlanModel";
import type { ProgramWorkspaceProps } from "./programWorkspaceTypes";
import { suggestedSafeZ } from "./safeStartModel";
import { useProgramWorkspace } from "./useProgramWorkspace";
const FirstCutAuthorizationDialog = lazy(async () => {
  const module = await import("./FirstCutAuthorizationDialog");
  return { default: module.FirstCutAuthorizationDialog };
});
export type { ProgramMachineContext } from "./programWorkspaceTypes";

export function ProgramWorkspace(props: ProgramWorkspaceProps) {
  const {
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
  } = useProgramWorkspace(props);
  return (
    <section className="program-workspace" aria-labelledby="program-title">
      <header className="program-header">
        <div className="program-identity">
          <span>Программа</span>
          <strong id="program-title">
            {program?.sourceName ?? "Предпросмотр G-code"}
          </strong>
        </div>
        <div className="program-actions">
          {program && (
            <div
              className="preview-view"
              role="group"
              aria-label="Вид траектории"
            >
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
              disabled={loading || senderCommandBusy || senderActive || safeStartContext !== undefined}
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
              disabled={loading || senderCommandBusy || senderActive}
              onClick={() => {
                setLoaded(undefined);
                setToolAssignments([]);
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
              disabled={!desktopRuntime || senderCommandBusy || senderActive}
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
              Safe Z {safeStartContext.package.safeZMm.toFixed(3)} mm ·{" "}
              {safeStartContext.package.workCoordinateSystem.toUpperCase()}
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
            cuttingDepthAdjustmentMm={
              depthCorrection.enabled ? depthCorrection.adjustmentMm : 0
            }
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
                !loading &&
                !senderCommandBusy &&
                !senderActive,
            )}
            selectedMotionCount={selectedMotionCount}
            selectedProgramLine={selectedProgramLine}
            selectedSourceLine={selectedSourceLine}
            toolCoordinateSystem={machineContext?.activeCoordinateSystem}
            toolPosition={machineContext?.workPosition}
            toolVisualization={toolVisualization}
            view={view}
          />

          <aside
            className="program-diagnostics"
            aria-label="Выполнение и диагностика программы"
          >
            {realRunTarget && (programRunVisible || checkRunVisible) ? (
              <ProgramRunCard
                busy={senderCommandBusy}
                checkAction={checkAction}
                checkControlsAvailable={realRunGateway !== undefined}
                checkRun={checkRunVisible}
                failureSummary={displayedSenderFailure}
                machineContextAvailable={machineContext !== undefined}
                onCancelCheck={() => {
                  if (realRunGateway)
                    void runSenderAction(realRunGateway.abortProgram);
                }}
                onPause={() => {
                  if (realRunGateway)
                    void runSenderAction(realRunGateway.pauseProgram);
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
                  if (realRunGateway)
                    void runSenderAction(realRunGateway.resumeProgram);
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
                    senderCommandBusy ||
                    surfaceMapBusy ||
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
                  onSurfaceMap={(enabled) =>
                    void setSurfaceMapApplication(enabled).catch(
                      () => undefined,
                    )
                  }
                  surfaceMap={
                    surfaceMap
                      ? {
                          checked:
                            surfaceMap.enabled &&
                            programExecutionOptions.surfaceMapId ===
                              surfaceMap.map.mapId,
                          detail: surfaceMap.detail,
                          disabled:
                            surfaceMapBusy ||
                            senderActive ||
                            !surfaceMap.coversProgram ||
                            !surfaceMap.usable,
                          warning:
                            !surfaceMap.coversProgram ||
                            !surfaceMap.usable ||
                            surfaceMap.suspiciousNeighborJump,
                        }
                      : undefined
                  }
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
                          void updateExecutionOption(
                            "optionalStop",
                            event.target.checked,
                          )
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
                          void updateExecutionOption(
                            "blockDelete",
                            event.target.checked,
                          )
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
          selectedCommand={
            selectedProgramLine.source || selectedProgramLine.normalized
          }
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

      {firstCutOpen && (
        <Suspense fallback={null}>
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
            open
            report={reportForProgram}
            startingToolNumber={
              program ? initialProgramToolNumber(program) : undefined
            }
            surfaceMap={
              surfaceMap
                ? {
                    mapId: surfaceMap.map.mapId,
                    enabled:
                      surfaceMap.enabled &&
                      programExecutionOptions.surfaceMapId ===
                        surfaceMap.map.mapId,
                    usable: surfaceMap.usable,
                    coversProgram: surfaceMap.coversProgram,
                    zRangeMm: surfaceMap.zRangeMm,
                    suspiciousNeighborJump: surfaceMap.suspiciousNeighborJump,
                    maximumNeighborDeltaMm: surfaceMap.maximumNeighborDeltaMm,
                    detail: surfaceMap.detail,
                    busy: surfaceMapBusy || senderCommandBusy,
                    onApply: applySurfaceMapFromFirstCut,
                  }
                : undefined
            }
          />
        </Suspense>
      )}

      {displayedSender.state === "toolChange" &&
        displayedSender.currentSourceLine !== undefined &&
        realRunGateway && (
          <ToolChangeDialog
            key={`${displayedSender.runSequence}:${displayedSender.currentSourceLine}:${displayedSender.requestedTool}`}
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

      {error && !onError && (
        <div className="program-error" role="alert">
          <span>{error}</span>
          <button
            aria-label="Закрыть сообщение"
            onClick={() => setError(undefined)}
            type="button"
          >
            ×
          </button>
        </div>
      )}
    </section>
  );
}
