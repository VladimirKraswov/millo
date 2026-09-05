import { Puzzle } from "lucide-react";
import { lazy, Suspense } from "react";
import {
  acknowledgeReset,
  confirmSoftReset,
  feedHold,
  refreshStatus,
  requestSoftReset,
  unlockAlarm,
} from "./api/controller";
import {
  developmentAuditFixture,
  developmentFirstCutFixture,
  developmentFixture,
  developmentPreflightFixture,
  developmentPreviewFixture,
} from "./app/developmentFixtures";
import { useWorkstation } from "./app/useWorkstation";
import { MachineStatusStrip } from "./app/workspace/MachineStatusStrip";
import { WorkspaceNavigation } from "./app/workspace/WorkspaceNavigation";
import { WorkspaceNotice } from "./app/workspace/WorkspaceNotice";
import { FeatureErrorBoundary } from "./components/FeatureErrorBoundary";
import { formatCoordinate } from "./components/PositionReadout";
import { SafetyControls } from "./components/SafetyControls";
import { WorkspaceToolsMenu } from "./components/WorkspaceToolsMenu";
import {
  connectionLabels,
  ConnectionPanel,
} from "./features/connection/ConnectionPanel";
import { ControllerInspector } from "./features/controller/ControllerInspector";
import { previewHeightmapGateway } from "./features/heightmap/previewHeightmapGateway";
import { MachineProfiles } from "./features/machine-profiles/MachineProfiles";
import { previewOperatorConsole } from "./features/operator-console/previewOperatorConsole";
import {
  previewFixtureCheckCompleteSender,
  previewFixtureCheckControlGateway,
  previewFixtureCheckRunningSender,
  previewFixtureCompletedSender,
  previewFixtureCutRunningSender,
  previewFixtureFirstCutGateway,
  previewFixtureProgramGateway,
  previewFixtureRecoveryGateway,
  previewFixtureToolChangeSender,
} from "./features/program/previewFixtureFirstCut";
import { previewFixturePreflightGateway } from "./features/program/previewFixturePreflight";
import { ProgramWorkspace } from "./features/program/ProgramWorkspace";
import { ScriptPluginContributions } from "./features/script-plugins/ScriptPluginContributions";
import { WorkZeroDialog } from "./features/work-zero/WorkZeroDialog";
import { tauriHeightmapGateway } from "./platform/machine/tauriHeightmapGateway";
import { tauriMachineCommandGateway } from "./platform/machine/tauriMachineCommandGateway";
import { tauriWorkCoordinateGateway } from "./platform/machine/tauriWorkCoordinateGateway";
import { tauriZProbeGateway } from "./platform/machine/tauriZProbeGateway";
import { tauriScriptPluginGateway } from "./platform/plugins/tauriScriptPluginGateway";
import { tauriProgramGateway } from "./platform/program/tauriProgramGateway";
import { tauriRealRunPreflightGateway } from "./platform/program/tauriRealRunPreflightGateway";
import { tauriSenderStateGateway } from "./platform/program/tauriSenderStateGateway";
import { isControllerStableIdle } from "./shared/controllerReadiness";
import { type WorkCoordinateSystem } from "./shared/machine";
const HelpDialog = lazy(async () => ({
  default: (await import("./features/help/HelpDialog")).HelpDialog,
}));
const MachineSettingsDialog = lazy(async () => ({
  default: (await import("./features/machine-settings/MachineSettingsDialog"))
    .MachineSettingsDialog,
}));
const DiagnosticLogViewer = lazy(async () => ({
  default: (await import("./features/diagnostics/DiagnosticLogViewer"))
    .DiagnosticLogViewer,
}));
const OperatorConsole = lazy(async () => ({
  default: (await import("./features/operator-console/OperatorConsole"))
    .OperatorConsole,
}));
const ZProbeDialog = lazy(async () => ({
  default: (await import("./features/probe/ZProbeDialog")).ZProbeDialog,
}));
const ToolLibraryDialog = lazy(async () => ({
  default: (await import("./features/tool-library/ToolLibraryDialog"))
    .ToolLibraryDialog,
}));
const ScriptPluginManager = lazy(async () => ({
  default: (await import("./features/script-plugins/ScriptPluginManager"))
    .ScriptPluginManager,
}));

export default function App() {
  const {
    baudRate,
    canDisconnect,
    developmentToolAssignments,
    discovering,
    hasConnection,
    likelyGrblOnly,
    machineSyncing,
    selectedTransport,
    visibleTransports,
    activeProgram,
    addMachineProfile,
    applicationPreferences,
    chooseMachineProfile,
    connectSelectedTransport,
    consoleOpen,
    controllerSettings,
    controlsBusy,
    desktopRuntime,
    detectSelectedMachine,
    disconnectController,
    discoverTransports,
    displayedTransport,
    effectiveMachineProfiles,
    generatedJob,
    helpOpen,
    inspecting,
    inspection,
    isConnected,
    logOpen,
    machineBound,
    maxJogDistanceMm,
    maxJogFeedMmPerMin,
    noticeError,
    onboardingDraft,
    pluginHost,
    probeEstablishedZDatum,
    profileBusy,
    readDeviceInspection,
    recoverConnectedMachine,
    rememberProbeEstablishedZDatum,
    returnToWorkOrigin,
    rollbackSetting,
    runAction,
    saveApplicationPreferences,
    scriptManagerOpen,
    scriptPlugins,
    selectedMachine,
    setActiveProgram,
    setBaudRate,
    setConsoleOpen,
    setHelpOpen,
    setInspection,
    setLikelyGrblOnly,
    setLogOpen,
    setMachineProfiles,
    setNoticeError,
    setOnboardingDraft,
    setScriptManagerOpen,
    setScriptPlugins,
    setSelectedTransportId,
    setSettingsFocus,
    setSettingsOpen,
    setToolLibraryOpen,
    setUiError,
    setWorkZeroOpen,
    setWorkbenchView,
    setZProbeOpen,
    settingsFocus,
    settingsOpen,
    snapshot,
    toolLibrary,
    toolLibraryOpen,
    transportLocked,
    updateLocalMachine,
    workPositionView,
    workZeroOpen,
    workbenchView,
    writeControllerSetting,
    zProbeOpen,
  } = useWorkstation();
  return (
    <div className="app-shell workstation">
      <header className="topbar">
        <div className="brand">
          <span className="brand-mark" aria-hidden="true">
            <i />
          </span>
          <div>
            <strong>Millo</strong>
            <span>Управление станком</span>
          </div>
        </div>

        <MachineProfiles
          busy={profileBusy}
          canDetect={desktopRuntime && !transportLocked && !controlsBusy}
          locked={transportLocked}
          onCreate={addMachineProfile}
          onDetect={detectSelectedMachine}
          onEdit={() => {
            setSettingsFocus("local");
            setSettingsOpen(true);
          }}
          onOnboardingDismiss={() => setOnboardingDraft(undefined)}
          onSelect={chooseMachineProfile}
          onboardingDraft={onboardingDraft}
          state={effectiveMachineProfiles}
        />

        <div className="topbar-tools" aria-label="Инструменты задания">
          <WorkspaceToolsMenu
            onExtensionError={(contributionId, error) =>
              setUiError(`Plugin UI ${contributionId}: ${String(error)}`)
            }
            registry={pluginHost.uiRegistry}
          />
          <button
            aria-label="Макросы и плагины"
            className="script-manager-trigger"
            disabled={!desktopRuntime}
            onClick={() => setScriptManagerOpen(true)}
            title="Макросы и плагины"
            type="button"
          >
            <Puzzle aria-hidden="true" size={16} />
          </button>
        </div>

        <div className={`connection-state is-${snapshot.connection}`}>
          <span className="state-dot" />
          <div>
            <small>{displayedTransport.label}</small>
            <strong>{connectionLabels[snapshot.connection]}</strong>
          </div>
        </div>
      </header>

      <MachineStatusStrip
        snapshot={snapshot}
        position={workPositionView.position}
        coordinateSystem={workPositionView.coordinateSystem}
        desktopRuntime={desktopRuntime}
        busy={controlsBusy}
        onProbe={() => setZProbeOpen(true)}
        onZero={() => setWorkZeroOpen(true)}
        onUnlock={() => void runAction(unlockAlarm)}
        onAcknowledgeReset={() => void runAction(acknowledgeReset)}
        onSnapshot={pluginHost.machineState.publish}
        onError={setUiError}
        onReset={() => setInspection(undefined)}
      />
      <main className="workspace">
        <WorkspaceNavigation
          view={workbenchView}
          onView={setWorkbenchView}
          onTools={() => setToolLibraryOpen(true)}
          onProbe={() => setZProbeOpen(true)}
          onLog={() => setLogOpen(true)}
          onHelp={() => setHelpOpen(true)}
          onSettings={() => {
            setSettingsFocus("local");
            setSettingsOpen(true);
          }}
        />
        <section
          className={`machine-panel is-${workbenchView}`}
          aria-label={
            workbenchView === "program"
              ? "Рабочая область задания"
              : "Настройки контроллера"
          }
        >
          <div
            className="workbench-panel"
            hidden={workbenchView !== "program"}
            id="program-workbench"
          >
            <FeatureErrorBoundary name="Задание" onError={setUiError}>
              <ProgramWorkspace
                onError={setUiError}
                desktopRuntime={desktopRuntime}
                senderGateway={
                  developmentFixture === "check-running"
                    ? previewFixtureCheckControlGateway
                    : desktopRuntime
                      ? tauriSenderStateGateway
                      : undefined
                }
                gateway={
                  developmentPreviewFixture
                    ? previewFixtureProgramGateway
                    : tauriProgramGateway
                }
                heightmapGateway={
                  developmentFixture === "heightmap"
                    ? previewHeightmapGateway
                    : desktopRuntime
                      ? tauriHeightmapGateway
                      : undefined
                }
                initialProgram={developmentPreviewFixture}
                initialSource={developmentPreviewFixture?.lines
                  .map((line) => line.source)
                  .join("\n")}
                initialToolAssignments={
                  developmentFirstCutFixture
                    ? developmentToolAssignments
                    : undefined
                }
                incomingJob={generatedJob}
                initialRunIntent={
                  developmentFixture === "check-complete" ||
                  developmentFixture === "run-complete" ||
                  developmentFixture === "tool-motion"
                    ? "cutting"
                    : undefined
                }
                initialSender={
                  developmentFixture === "tool-change"
                    ? previewFixtureToolChangeSender
                    : developmentFixture === "run-complete"
                      ? previewFixtureCompletedSender
                      : developmentFixture === "check-running"
                        ? previewFixtureCheckRunningSender
                        : developmentFixture === "tool-motion"
                          ? previewFixtureCutRunningSender
                          : developmentFixture === "check-complete"
                            ? previewFixtureCheckCompleteSender
                            : undefined
                }
                machineContext={{
                  activeCoordinateSystem: workPositionView.coordinateSystem,
                  busy: controlsBusy,
                  machineBound,
                  machineName:
                    selectedMachine?.name ?? displayedTransport.label,
                  machineProfileId: selectedMachine?.id,
                  machineSyncing,
                  onAcknowledgeReset: () => runAction(acknowledgeReset),
                  onConnect: connectSelectedTransport,
                  onOpenWorkZero: () => setWorkZeroOpen(true),
                  onReturnToWorkOrigin: returnToWorkOrigin,
                  onSyncMachine: recoverConnectedMachine,
                  onUnlock: () => runAction(unlockAlarm),
                  snapshot,
                  workPosition: workPositionView.position,
                }}
                onInspection={setInspection}
                onProgramChange={setActiveProgram}
                realRunAvailable={
                  developmentPreflightFixture ||
                  (desktopRuntime &&
                    isControllerStableIdle(snapshot) &&
                    machineBound)
                }
                realRunGateway={
                  developmentFixture === "recovery"
                    ? previewFixtureRecoveryGateway
                    : developmentFirstCutFixture
                      ? previewFixtureFirstCutGateway
                      : developmentPreflightFixture
                        ? previewFixturePreflightGateway
                        : desktopRuntime
                          ? tauriRealRunPreflightGateway
                          : undefined
                }
                realRunTarget={developmentPreflightFixture || desktopRuntime}
                tools={toolLibrary.tools}
              />
            </FeatureErrorBoundary>
          </div>

          <div
            className="workbench-panel"
            hidden={workbenchView !== "controller"}
            id="controller-workbench"
          >
            <ControllerInspector
              busy={controlsBusy}
              connected={isConnected}
              inspecting={inspecting}
              inspection={inspection}
              onRead={() => void readDeviceInspection()}
            />
          </div>
        </section>

        <ConnectionPanel
          actions={{
            onBaudRate: setBaudRate,
            onConnect: () => void connectSelectedTransport(),
            onDisconnect: () => void disconnectController(),
            onDismissError: () => setUiError(undefined),
            onLikelyGrblOnly: setLikelyGrblOnly,
            onOpenLog: () => setLogOpen(true),
            onOpenConsole: () => setConsoleOpen(true),
            onRefreshStatus: () => void runAction(refreshStatus),
            onRefreshTransports: () => void discoverTransports(),
            onTransport: setSelectedTransportId,
          }}
          controls={
            <SafetyControls
              desktopRuntime={
                desktopRuntime || developmentFixture === "machine-control"
              }
              extensionRegistry={pluginHost.uiRegistry}
              machineGateway={tauriMachineCommandGateway}
              workCoordinateGateway={tauriWorkCoordinateGateway}
              onError={setUiError}
              onInspection={setInspection}
              onOpenMotionSettings={() => {
                setSettingsFocus("motion");
                setSettingsOpen(true);
              }}
              onSnapshot={pluginHost.machineState.publish}
              snapshot={snapshot}
              machineBound={machineBound}
              maxJogDistanceMm={maxJogDistanceMm}
              maxJogFeedMmPerMin={maxJogFeedMmPerMin}
              useProbeForZ={probeEstablishedZDatum !== undefined}
              homingInstalled={selectedMachine?.homingInstalled ?? false}
              spindleControl={selectedMachine?.spindleControl ?? "manual"}
              floodCoolantControl={
                selectedMachine?.floodCoolantControl ?? false
              }
              mistCoolantControl={selectedMachine?.mistCoolantControl ?? false}
              activeCoordinateSystem={
                workPositionView.coordinateSystem.toLowerCase() as WorkCoordinateSystem
              }
              rotaryAxis={selectedMachine?.rotaryAxis}
            />
          }
          view={{
            baudRate,
            canDisconnect,
            controlsBusy,
            desktopRuntime,
            discovering,
            displayedError: undefined,
            displayedTransport,
            hasConnection,
            isConnected,
            likelyGrblOnly,
            safeCommandMode: applicationPreferences.safeCommandMode,
            selectedMachineName: selectedMachine?.name,
            selectedTransport,
            snapshot,
            transportLocked,
            visibleTransports,
          }}
        />
      </main>
      <footer className="workspace-statusbar">
        <span>{selectedMachine?.name ?? "Станок не выбран"}</span>
        <span>
          Подача <b>{snapshot.machine.feedRate.toFixed(0)}</b> мм/мин
        </span>
        <span>
          Шпиндель <b>{snapshot.machine.spindleSpeed.toFixed(0)}</b> об/мин
        </span>
        <span title="Машинные координаты">
          G53 · X {formatCoordinate(snapshot.machine.machinePosition?.x)} · Y{" "}
          {formatCoordinate(snapshot.machine.machinePosition?.y)} · Z{" "}
          {formatCoordinate(snapshot.machine.machinePosition?.z)}
        </span>
      </footer>
      <WorkspaceNotice
        message={noticeError}
        onDismiss={() => setNoticeError(undefined)}
        onLog={() => setLogOpen(true)}
      />
      <Suspense fallback={null}>
        {helpOpen && <HelpDialog onClose={() => setHelpOpen(false)} />}
      </Suspense>

      <Suspense fallback={null}>
        {settingsOpen && (
          <MachineSettingsDialog
            applicationPreferences={applicationPreferences}
            initialQuery={settingsFocus === "motion" ? "acceleration" : ""}
            initialView={settingsFocus === "motion" ? "controller" : "local"}
            onClose={() => setSettingsOpen(false)}
            onApplicationPreferencesUpdate={saveApplicationPreferences}
            onLocalUpdate={updateLocalMachine}
            onOpenToolLibrary={() => {
              setSettingsOpen(false);
              setToolLibraryOpen(true);
            }}
            onRollback={rollbackSetting}
            onWrite={writeControllerSetting}
            open
            profile={selectedMachine}
            settings={controllerSettings}
          />
        )}
        {logOpen && (
          <DiagnosticLogViewer
            desktopRuntime={
              desktopRuntime || developmentFixture === "heightmap"
            }
            initialSnapshot={
              developmentFixture === "logs"
                ? developmentAuditFixture
                : undefined
            }
            onClose={() => setLogOpen(false)}
            onError={setUiError}
            open
          />
        )}
        {consoleOpen && (
          <OperatorConsole
            desktopRuntime={desktopRuntime || developmentFixture === "console"}
            execute={
              developmentFixture === "console"
                ? (command) =>
                    previewOperatorConsole(
                      command,
                      snapshot,
                      applicationPreferences.safeCommandMode,
                    )
                : undefined
            }
            onClose={() => setConsoleOpen(false)}
            onSnapshot={pluginHost.machineState.publish}
            open
            safeCommandMode={applicationPreferences.safeCommandMode}
            snapshot={snapshot}
          />
        )}
      </Suspense>

      <WorkZeroDialog
        activeCoordinateSystem={workPositionView.coordinateSystem}
        desktopRuntime={desktopRuntime}
        disabled={controlsBusy}
        gateway={tauriWorkCoordinateGateway}
        onClose={() => setWorkZeroOpen(false)}
        onError={setUiError}
        onSnapshot={pluginHost.machineState.publish}
        open={workZeroOpen}
        position={workPositionView.position}
        snapshot={snapshot}
        useProbeForZ={probeEstablishedZDatum !== undefined}
      />
      <Suspense fallback={null}>
        {zProbeOpen && (
          <ZProbeDialog
            activeCoordinateSystem={
              workPositionView.coordinateSystem.toLowerCase() as WorkCoordinateSystem
            }
            desktopRuntime={
              desktopRuntime || developmentFixture === "heightmap"
            }
            disabled={controlsBusy}
            gateway={tauriZProbeGateway}
            heightmapGateway={
              developmentFixture === "heightmap"
                ? previewHeightmapGateway
                : tauriHeightmapGateway
            }
            machineTravel={selectedMachine?.travelMm}
            onAbort={async () => {
              await feedHold();
              const challenge = await requestSoftReset();
              return confirmSoftReset(challenge.id);
            }}
            onClose={() => setZProbeOpen(false)}
            onError={setUiError}
            onSaveSettings={async (settings) => {
              if (!selectedMachine)
                throw new Error("Сначала выберите профиль станка");
              if (developmentFixture === "heightmap" && !desktopRuntime) {
                setMachineProfiles((current) => ({
                  ...current,
                  profiles: current.profiles.map((profile) =>
                    profile.id === selectedMachine.id
                      ? {
                          ...profile,
                          probeInstalled: true,
                          probeSettings: settings,
                        }
                      : profile,
                  ),
                }));
                return;
              }
              await updateLocalMachine({
                ...selectedMachine,
                probeInstalled: true,
                probeSettings: settings,
              });
            }}
            onSnapshot={pluginHost.machineState.publish}
            onZeroEstablished={rememberProbeEstablishedZDatum}
            onUnlock={unlockAlarm}
            open
            profileId={selectedMachine?.id}
            program={activeProgram}
            probeInstalled={selectedMachine?.probeInstalled ?? false}
            settings={selectedMachine?.probeSettings}
            snapshot={snapshot}
          />
        )}
        {pluginHost.tools && toolLibraryOpen && (
          <ToolLibraryDialog
            onClose={() => setToolLibraryOpen(false)}
            open
            service={pluginHost.tools}
          />
        )}
      </Suspense>
      <ScriptPluginContributions
        gateway={tauriScriptPluginGateway}
        jobs={pluginHost.generatedJobs}
        machine={pluginHost.machineState}
        onError={setUiError}
        plugins={scriptPlugins}
        registry={pluginHost.uiRegistry}
      />
      <Suspense fallback={null}>
        {scriptManagerOpen && (
          <ScriptPluginManager
            gateway={tauriScriptPluginGateway}
            onChange={setScriptPlugins}
            onClose={() => setScriptManagerOpen(false)}
            onError={setUiError}
            open
            plugins={scriptPlugins}
          />
        )}
      </Suspense>
    </div>
  );
}
