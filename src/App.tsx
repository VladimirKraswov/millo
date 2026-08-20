import {
  lazy,
  Suspense,
  useEffect,
  useMemo,
  useRef,
  useState,
  useSyncExternalStore,
} from "react";
import { KeyRound, Puzzle } from "lucide-react";

import { bootstrapPluginHost } from "./app/bootstrapPluginHost";
import { DeferredDisposal } from "./app/DeferredDisposal";
import {
  developmentAuditFixture,
  developmentFirstCutFixture,
  developmentFixture,
  developmentJogSnapshot,
  developmentMachineFixture,
  developmentPreflightFixture,
  developmentPreviewFixture,
  developmentProbeFixture,
  developmentProfileFixture,
  developmentSettingsFixture,
} from "./app/developmentFixtures";
import { previewPcbImageJobGateway } from "./app/previewPcbImageJobGateway";
import { CapabilityGrantStore } from "./platform/plugins/CapabilityGrantStore";
import { createImageToGcodePlugin, IMAGE_TO_GCODE_PLUGIN_ID } from "./plugins/image-to-gcode/createImageToGcodePlugin";
import {
  createPcbFabricationPlugin,
  PCB_FABRICATION_PLUGIN_ID,
} from "./plugins/pcb-fabrication/createPcbFabricationPlugin";
import {
  createSpoilboardSurfacingPlugin,
  SPOILBOARD_SURFACING_PLUGIN_ID,
} from "./plugins/spoilboard-surfacing/createSpoilboardSurfacingPlugin";
import {
  acknowledgeReset,
  connectTransport,
  confirmSoftReset,
  disconnect,
  feedHold,
  getActiveTransport,
  getControllerSettings,
  inspectDevice,
  isDesktopRuntime,
  listTransports,
  refreshStatus,
  requestSoftReset,
  rollbackControllerSetting,
  unlockAlarm,
  updateControllerSetting,
} from "./api/controller";
import {
  createMachineProfile,
  detectMachineProfile,
  getMachineProfiles,
  selectMachineProfile,
  updateMachineLocalSettings,
} from "./api/profiles";
import { formatCoordinate, PositionReadout } from "./components/PositionReadout";
import { SafetyControls } from "./components/SafetyControls";
import { ControllerInspector } from "./features/controller/ControllerInspector";
import {
  ConnectionPanel,
  connectionLabels,
} from "./features/connection/ConnectionPanel";
import { previewFixturePreflightGateway } from "./features/program/previewFixturePreflight";
import {
  previewFixtureCheckCompleteSender,
  previewFixtureCompletedSender,
  previewFixtureCheckControlGateway,
  previewFixtureCheckRunningSender,
  previewFixtureCutRunningSender,
  previewFixtureFirstCutGateway,
  previewFixtureProgramGateway,
  previewFixtureRecoveryGateway,
  previewFixtureToolChangeSender,
} from "./features/program/previewFixtureFirstCut";
import { ProgramWorkspace } from "./features/program/ProgramWorkspace";
import { MachineProfiles } from "./features/machine-profiles/MachineProfiles";
import { ProbeIndicator } from "./features/probe/ProbeIndicator";
import { previewHeightmapGateway } from "./features/heightmap/previewHeightmapGateway";
import { heightmapHasCurrentZDatum } from "./features/heightmap/heightmapModel";
import { WorkZeroDialog } from "./features/work-zero/WorkZeroDialog";
import { WorkspaceToolsMenu } from "./components/WorkspaceToolsMenu";
import { ScriptPluginContributions } from "./features/script-plugins/ScriptPluginContributions";
import { previewFixtureScriptPlugins } from "./features/script-plugins/previewFixtureScriptPlugins";
import { previewToolLibraryGateway } from "./features/tool-library/previewToolLibraryGateway";
import { resolveWorkPosition } from "./features/work-zero/workPositionModel";
import { bindMachineStateStream } from "./platform/machine/MachineStateEventStream";
import { tauriMachineCommandGateway } from "./platform/machine/tauriMachineCommandGateway";
import { tauriMachineStateEventStream } from "./platform/machine/tauriMachineStateEventStream";
import { tauriWorkCoordinateGateway } from "./platform/machine/tauriWorkCoordinateGateway";
import { tauriZProbeGateway } from "./platform/machine/tauriZProbeGateway";
import { tauriHeightmapGateway } from "./platform/machine/tauriHeightmapGateway";
import { tauriProgramGateway } from "./platform/program/tauriProgramGateway";
import { tauriImageJobGateway } from "./platform/jobs/tauriImageJobGateway";
import { tauriToolLibraryGateway } from "./platform/tooling/tauriToolLibraryGateway";
import { tauriScriptPluginGateway } from "./platform/plugins/tauriScriptPluginGateway";
import { tauriSenderStateGateway } from "./platform/program/tauriSenderStateGateway";
import { tauriRealRunPreflightGateway } from "./platform/program/tauriRealRunPreflightGateway";
import {
  emptySnapshot,
  type ControllerSnapshot,
  type HardwareInspection,
  type TransportDescriptor,
  type WorkCoordinateSystem,
  type ZProbeOutcome,
} from "./shared/machine";
import {
  hasControllerSession,
  isControllerConnected,
  isControllerStableIdle,
} from "./shared/controllerReadiness";
import type { GcodeProgram } from "./shared/program";
import type {
  MachineProfile,
  MachineProfileDraft,
  MachineProfileState,
} from "./shared/profile";
import { selectedMachineProfile } from "./shared/profile";
import type {
  ControllerSettingEditRequest,
  ControllerSettingsState,
} from "./shared/settings";
import type { InstalledScriptPlugin } from "./shared/scriptPlugins";
import { emptyToolLibrary } from "./shared/tooling";

const MachineSettingsDialog = lazy(async () => ({
  default: (await import("./features/machine-settings/MachineSettingsDialog"))
    .MachineSettingsDialog,
}));
const DiagnosticLogViewer = lazy(async () => ({
  default: (await import("./features/diagnostics/DiagnosticLogViewer"))
    .DiagnosticLogViewer,
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

const subscribeEmptyToolLibrary = () => () => undefined;
const readEmptyToolLibrary = () => emptyToolLibrary;
const developmentToolAssignments = Object.freeze([
  Object.freeze({ toolNumber: 1, toolId: "preset-xc-nlj3-2001" }),
]);

const disconnectedTransport: TransportDescriptor = {
  id: "",
  kind: "serial",
  label: "Serial controller",
  likelyGrbl: false,
};

interface ProbeEstablishedZDatum {
  readonly coordinateSystem: WorkCoordinateSystem;
  readonly resetCount: number;
  readonly reconnectCount: number;
  readonly source: "probe" | "heightmap";
  readonly workCoordinateOffsetZ?: number;
}

export default function App() {
  const pluginHost = useMemo(
    () =>
      bootstrapPluginHost({
        initialSnapshot: developmentMachineFixture ? developmentJogSnapshot : emptySnapshot,
        machineCommands: tauriMachineCommandGateway,
        workCoordinates: tauriWorkCoordinateGateway,
        imageJobs: developmentFixture === "pcb" ? previewPcbImageJobGateway : tauriImageJobGateway,
        toolLibrary: isDesktopRuntime()
          ? tauriToolLibraryGateway
          : previewToolLibraryGateway,
        grants: new CapabilityGrantStore([
          {
            pluginId: IMAGE_TO_GCODE_PLUGIN_ID,
            capabilities: ["ui.contribute", "jobs.create"],
          },
          {
            pluginId: SPOILBOARD_SURFACING_PLUGIN_ID,
            capabilities: ["ui.contribute", "jobs.create", "tools.read"],
          },
          {
            pluginId: PCB_FABRICATION_PLUGIN_ID,
            capabilities: ["ui.contribute", "jobs.create", "tools.read"],
          },
        ]),
        bundledPlugins: [
          createImageToGcodePlugin({ initialOpen: developmentFixture === "image-job" }),
          createPcbFabricationPlugin({ initialOpen: developmentFixture === "pcb" }),
          createSpoilboardSurfacingPlugin({
            initialOpen: developmentFixture === "surfacing",
          }),
        ],
      }),
    [],
  );
  const snapshot = useSyncExternalStore(
    pluginHost.machineState.subscribe,
    pluginHost.machineState.current,
    pluginHost.machineState.current,
  );
  const [transports, setTransports] = useState<TransportDescriptor[]>([]);
  const [selectedTransportId, setSelectedTransportId] = useState("");
  const [activeTransport, setActiveTransport] =
    useState<TransportDescriptor>(disconnectedTransport);
  const [baudRate, setBaudRate] = useState(115_200);
  const [likelyGrblOnly, setLikelyGrblOnly] = useState(true);
  const [inspection, setInspection] = useState<HardwareInspection>();
  const [machineProfiles, setMachineProfiles] = useState<MachineProfileState>(
    developmentFixture === "profiles" ||
    developmentFixture === "settings" ||
    developmentMachineFixture
      ? developmentProfileFixture
      : { profiles: [] },
  );
  const [profileBusy, setProfileBusy] = useState(false);
  const [machineSyncing, setMachineSyncing] = useState(false);
  const [machineSyncAttempted, setMachineSyncAttempted] = useState(false);
  const [controllerSettings, setControllerSettings] =
    useState<ControllerSettingsState | undefined>(
      developmentFixture === "settings" || developmentMachineFixture
        ? developmentSettingsFixture
        : undefined,
    );
  const [onboardingDraft, setOnboardingDraft] = useState<MachineProfileDraft>();
  const [activeProgram, setActiveProgram] = useState<GcodeProgram>();
  const [settingsOpen, setSettingsOpen] = useState(developmentFixture === "settings");
  const [settingsFocus, setSettingsFocus] = useState<"local" | "motion">("local");
  const [inspecting, setInspecting] = useState(false);
  const [busy, setBusy] = useState(false);
  const [discovering, setDiscovering] = useState(false);
  const [uiError, setUiError] = useState<string>();
  const [logOpen, setLogOpen] = useState(developmentFixture === "logs");
  const [workZeroOpen, setWorkZeroOpen] = useState(false);
  const [zProbeOpen, setZProbeOpen] = useState(developmentProbeFixture);
  const [probeEstablishedZDatum, setProbeEstablishedZDatum] =
    useState<ProbeEstablishedZDatum>();
  const zDatumRestoreKey = useRef<string | undefined>(undefined);
  const [toolLibraryOpen, setToolLibraryOpen] = useState(
    developmentFixture === "tools",
  );
  const [scriptManagerOpen, setScriptManagerOpen] = useState(
    developmentFixture === "plugins",
  );
  const [scriptPlugins, setScriptPlugins] = useState<
    readonly InstalledScriptPlugin[]
  >(developmentFixture === "plugins" ? previewFixtureScriptPlugins : []);
  const [workbenchView, setWorkbenchView] = useState<"program" | "controller">(
    "program",
  );
  const desktopRuntime = useMemo(isDesktopRuntime, []);
  const generatedJob = useSyncExternalStore(
    pluginHost.generatedJobs.subscribe,
    pluginHost.generatedJobs.current,
    pluginHost.generatedJobs.current,
  );
  const toolLibrary = useSyncExternalStore(
    pluginHost.tools?.subscribe ?? subscribeEmptyToolLibrary,
    pluginHost.tools?.current ?? readEmptyToolLibrary,
    pluginHost.tools?.current ?? readEmptyToolLibrary,
  );
  const pluginHostLifecycle = useMemo(
    () => new DeferredDisposal(
      () => pluginHost.dispose(),
      (error) => setUiError(String(error)),
    ),
    [pluginHost],
  );

  useEffect(() => {
    void pluginHost.ready.catch((error: unknown) => setUiError(String(error)));
    return pluginHostLifecycle.mount();
  }, [pluginHost, pluginHostLifecycle]);

  useEffect(() => {
    if (!desktopRuntime) return;
    void tauriScriptPluginGateway
      .list()
      .then(setScriptPlugins)
      .catch((error: unknown) => setUiError(String(error)));
  }, [desktopRuntime]);

  const synchronizeConnectedMachine = async (): Promise<boolean> => {
    if (!desktopRuntime || snapshot.connection !== "connected") return false;
    setMachineSyncAttempted(true);
    setMachineSyncing(true);
    setUiError(undefined);
    try {
      const [settings, profiles] = await Promise.all([
        getControllerSettings(),
        getMachineProfiles(),
      ]);
      setControllerSettings(settings);
      setMachineProfiles(settings.profileId
        ? { ...profiles, selectedProfileId: settings.profileId }
        : profiles);
      if (!settings.profileId) {
        throw new Error(
          "Подключённый контроллер не привязан к профилю. Отключите станок и подключите его снова для автоматического определения.",
        );
      }
      return true;
    } catch (error) {
      setUiError(String(error));
      return false;
    } finally {
      setMachineSyncing(false);
    }
  };

  useEffect(() => {
    if (
      !desktopRuntime ||
      snapshot.connection !== "connected" ||
      controllerSettings !== undefined ||
      machineSyncAttempted ||
      machineSyncing
    ) return;
    void synchronizeConnectedMachine();
  }, [controllerSettings, desktopRuntime, machineSyncAttempted, machineSyncing, snapshot.connection]);

  useEffect(() => {
    if (snapshot.connection === "disconnected") {
      setMachineSyncAttempted(false);
      setMachineSyncing(false);
    }
  }, [snapshot.connection]);

  const rememberProbeEstablishedZDatum = (
    outcome: ZProbeOutcome,
    source: ProbeEstablishedZDatum["source"],
  ) => {
    setProbeEstablishedZDatum({
      coordinateSystem: outcome.coordinateSystem,
      resetCount: outcome.snapshot.resetCount,
      reconnectCount: outcome.snapshot.reconnectCount,
      source,
      workCoordinateOffsetZ: outcome.snapshot.machine.workCoordinateOffset?.z,
    });
  };

  useEffect(() => {
    if (generatedJob) setWorkbenchView("program");
  }, [generatedJob]);

  useEffect(() => {
    if (!desktopRuntime) {
      return;
    }

    let active = true;
    const unbindMachineState = bindMachineStateStream({
      stream: tauriMachineStateEventStream,
      store: pluginHost.machineState,
      onSnapshot: (value) => {
        if (!active) return;
        if (
          value.connection !== "connected" ||
          value.machine.mode !== "idle" ||
          value.alarm !== undefined ||
          value.resetNotice !== undefined
        ) {
          setInspection(undefined);
        }
      },
      onError: (error) => {
        if (active) setUiError(String(error));
      },
    });
    void getActiveTransport()
      .then((value) => {
        if (active) {
          setActiveTransport(value);
          setSelectedTransportId(value.id);
        }
      })
      .catch((error: unknown) => {
        if (active) setUiError(String(error));
      });
    void listTransports()
      .then((value) => {
        if (active) setTransports(value);
      })
      .catch((error: unknown) => {
        if (active) setUiError(String(error));
      });
    void getMachineProfiles()
      .then((value) => {
        if (active) setMachineProfiles(value);
      })
      .catch((error: unknown) => {
        if (active) setUiError(String(error));
      });
    return () => {
      active = false;
      unbindMachineState();
    };
  }, [desktopRuntime, pluginHost]);

  const runAction = async (
    action: () => Promise<ControllerSnapshot>,
  ): Promise<boolean> => {
    setBusy(true);
    setUiError(undefined);
    try {
      pluginHost.machineState.publish(await action());
      return true;
    } catch (error) {
      setUiError(String(error));
      return false;
    } finally {
      setBusy(false);
    }
  };

  const returnToWorkOrigin = async (clearanceZMm: number): Promise<void> => {
    setBusy(true);
    setUiError(undefined);
    try {
      const outcome = await tauriWorkCoordinateGateway.returnToOrigin!({
        clearanceZMm,
        xyFeedMmPerMin: 300,
        zFeedMmPerMin: 100,
      });
      pluginHost.machineState.publish(outcome.snapshot);
    } catch (error) {
      setUiError(String(error));
      throw error;
    } finally {
      setBusy(false);
    }
  };

  const discoverTransports = async () => {
    setDiscovering(true);
    setUiError(undefined);
    try {
      const discovered = await listTransports();
      setTransports(
        activeTransport.kind === "serial" &&
          !discovered.some((transport) => transport.id === activeTransport.id)
          ? [...discovered, activeTransport]
          : discovered,
      );
    } catch (error) {
      setUiError(String(error));
    } finally {
      setDiscovering(false);
    }
  };

  const refreshMachineProfiles = async () => {
    if (!desktopRuntime) return;
    try {
      setMachineProfiles(await getMachineProfiles());
    } catch (error) {
      setUiError(String(error));
    }
  };

  const isConnected = isControllerConnected(snapshot);
  const hasConnection = hasControllerSession(snapshot);
  const canDisconnect =
    snapshot.connection !== "disconnected" && snapshot.connection !== "connecting";
  const transportLocked = hasConnection || snapshot.connection === "connecting";
  const visibleTransports = transports.filter(
    (transport) =>
      !likelyGrblOnly ||
      transport.likelyGrbl ||
      (transportLocked && transport.id === activeTransport.id),
  );
  const selectedTransport =
    visibleTransports.find((transport) => transport.id === selectedTransportId) ??
    visibleTransports[0] ??
    activeTransport;
  const displayedTransport = transportLocked ? activeTransport : selectedTransport;
  const displayedError = uiError ?? snapshot.lastError;
  const controlsBusy = busy || inspecting || profileBusy || machineSyncing;
  const effectiveMachineProfiles =
    hasConnection && controllerSettings
      ? {
          ...machineProfiles,
          selectedProfileId: controllerSettings.profileId,
        }
      : machineProfiles;
  const selectedMachine = selectedMachineProfile(effectiveMachineProfiles);
  const machineBound =
    controllerSettings?.profileId !== undefined &&
    selectedMachine?.id === controllerSettings.profileId;
  const jogAxisRates = ["$110", "$111", "$112"]
    .map((key) =>
      Number(
        controllerSettings?.snapshot.values.find((setting) => setting.key === key)
          ?.value,
      ),
    )
    .filter((value) => Number.isFinite(value) && value >= 10);
  const maxJogFeedMmPerMin =
    jogAxisRates.length > 0 ? Math.min(...jogAxisRates) : 1_000;
  const maxJogDistanceMm = selectedMachine?.maxJogDistanceMm ?? 50;
  const workPositionView = resolveWorkPosition(snapshot, inspection);

  useEffect(() => {
    if (!probeEstablishedZDatum) return;
    if (
      snapshot.connection !== "connected" ||
      snapshot.resetCount !== probeEstablishedZDatum.resetCount ||
      snapshot.reconnectCount !== probeEstablishedZDatum.reconnectCount ||
      workPositionView.coordinateSystem.toLowerCase() !==
        probeEstablishedZDatum.coordinateSystem.toLowerCase() ||
      (
        probeEstablishedZDatum.workCoordinateOffsetZ !== undefined &&
        snapshot.machine.workCoordinateOffset !== undefined &&
        Math.abs(
          probeEstablishedZDatum.workCoordinateOffsetZ -
          snapshot.machine.workCoordinateOffset.z,
        ) > 0.01
      )
    ) {
      setProbeEstablishedZDatum(undefined);
    }
  }, [
    probeEstablishedZDatum,
    snapshot.connection,
    snapshot.machine.workCoordinateOffset,
    snapshot.reconnectCount,
    snapshot.resetCount,
    workPositionView.coordinateSystem,
  ]);

  useEffect(() => {
    if (
      !desktopRuntime ||
      probeEstablishedZDatum ||
      snapshot.connection !== "connected" ||
      !selectedMachine?.id
    ) return;
    const profileId = selectedMachine.id;
    const offset = snapshot.machine.workCoordinateOffset;
    const restoreKey = [
      profileId,
      snapshot.resetCount,
      snapshot.reconnectCount,
      workPositionView.coordinateSystem,
      offset?.x,
      offset?.y,
      offset?.z,
    ].join(":");
    if (zDatumRestoreKey.current === restoreKey) return;
    zDatumRestoreKey.current = restoreKey;
    let active = true;
    void tauriHeightmapGateway.getSession().then((session) => {
      const stored = session.active;
      const binding = stored?.map.coordinateBinding;
      const currentOffset = snapshot.machine.workCoordinateOffset;
      if (
        !active ||
        stored?.machineProfileId !== profileId ||
        !binding ||
        !currentOffset ||
        !heightmapHasCurrentZDatum(
          stored.map,
          session.coordinateBindingStale,
          workPositionView.coordinateSystem.toLowerCase() as WorkCoordinateSystem,
          currentOffset,
        )
      ) return;
      setProbeEstablishedZDatum({
        coordinateSystem: binding.coordinateSystem,
        resetCount: snapshot.resetCount,
        reconnectCount: snapshot.reconnectCount,
        source: "heightmap",
        workCoordinateOffsetZ: currentOffset.z,
      });
    }).catch(() => undefined);
    return () => { active = false; };
  }, [
    desktopRuntime,
    probeEstablishedZDatum,
    selectedMachine?.id,
    snapshot.connection,
    snapshot.machine.workCoordinateOffset,
    snapshot.reconnectCount,
    snapshot.resetCount,
    workPositionView.coordinateSystem,
  ]);

  useEffect(() => {
    if (transportLocked || !selectedMachine?.connection) return;
    setSelectedTransportId(selectedMachine.connection.transportId);
    setBaudRate(selectedMachine.connection.baudRate);
  }, [selectedMachine, transportLocked]);

  const readDeviceInspection = async () => {
    setInspecting(true);
    setUiError(undefined);
    try {
      setInspection(await inspectDevice());
      setControllerSettings(await getControllerSettings());
    } catch (error) {
      setUiError(String(error));
    } finally {
      setInspecting(false);
    }
  };

  const connectSelectedTransport = async () => {
    if (!selectedTransport.id) {
      setUiError("Последовательный порт не выбран");
      return;
    }
    setInspection(undefined);
    setControllerSettings(undefined);
    setOnboardingDraft(undefined);
    setBusy(true);
    setMachineSyncAttempted(true);
    setUiError(undefined);
    try {
      const outcome = await connectTransport(selectedTransport.id, baudRate);
      pluginHost.machineState.publish(outcome.snapshot);
      setActiveTransport(selectedTransport);
      setInspection(outcome.inspection);
      setControllerSettings(outcome.settings);
      setMachineProfiles(
        outcome.onboardingDraft
          ? { ...outcome.profiles, selectedProfileId: undefined }
          : outcome.profiles,
      );
      setOnboardingDraft(outcome.onboardingDraft);
    } catch (error) {
      setUiError(String(error));
    } finally {
      setBusy(false);
    }
  };

  const disconnectController = async () => {
    const disconnected = await runAction(disconnect);
    if (disconnected) {
      setInspection(undefined);
      setControllerSettings(undefined);
      setOnboardingDraft(undefined);
      setSettingsOpen(false);
      await refreshMachineProfiles();
    }
  };

  const recoverConnectedMachine = async (): Promise<void> => {
    if (await synchronizeConnectedMachine()) return;
    await disconnectController();
    await connectSelectedTransport();
  };

  const chooseMachineProfile = async (profileId: string) => {
    setProfileBusy(true);
    setUiError(undefined);
    try {
      if (desktopRuntime) {
        setMachineProfiles(await selectMachineProfile(profileId));
      } else {
        setMachineProfiles((current) => ({
          ...current,
          selectedProfileId: profileId,
        }));
      }
      setInspection(undefined);
    } catch (error) {
      setUiError(String(error));
      throw error;
    } finally {
      setProfileBusy(false);
    }
  };

  const addMachineProfile = async (draft: MachineProfileDraft) => {
    setProfileBusy(true);
    setUiError(undefined);
    try {
      if (desktopRuntime) {
        setMachineProfiles(await createMachineProfile(draft));
        if (hasConnection) setControllerSettings(await getControllerSettings());
      } else if (developmentFixture === "profiles") {
        const id = `machine-${String(machineProfiles.profiles.length + 1).padStart(4, "0")}`;
        setMachineProfiles((current) => ({
          profiles: [...current.profiles, { ...draft, id }],
          selectedProfileId: id,
        }));
      } else {
        throw new Error("Machine profiles require the desktop runtime");
      }
      setInspection(undefined);
      setOnboardingDraft(undefined);
    } catch (error) {
      setUiError(String(error));
      throw error;
    } finally {
      setProfileBusy(false);
    }
  };

  const updateLocalMachine = async (
    profile: MachineProfile,
  ): Promise<MachineProfileState> => {
    const next = await updateMachineLocalSettings(profile.id, {
      name: profile.name,
      maxJogDistanceMm: profile.maxJogDistanceMm,
      rotaryAxis: profile.rotaryAxis,
      spindleControl: profile.spindleControl,
      floodCoolantControl: profile.floodCoolantControl,
      mistCoolantControl: profile.mistCoolantControl,
      homingInstalled: profile.homingInstalled,
      limitSwitchesInstalled: profile.limitSwitchesInstalled,
      probeInstalled: profile.probeInstalled,
      probeSettings: profile.probeSettings,
      emergencyStopInstalled: profile.emergencyStopInstalled,
    });
    setMachineProfiles(next);
    return next;
  };

  const writeControllerSetting = async (
    request: ControllerSettingEditRequest,
  ): Promise<ControllerSettingsState> => {
    const next = await updateControllerSetting(request);
    setControllerSettings(next);
    await refreshMachineProfiles();
    return next;
  };

  const rollbackSetting = async (
    key: string,
    revision: number,
  ): Promise<ControllerSettingsState> => {
    const next = await rollbackControllerSetting(key, revision);
    setControllerSettings(next);
    await refreshMachineProfiles();
    return next;
  };

  const detectSelectedMachine = async (): Promise<MachineProfileDraft> => {
    if (!desktopRuntime) {
      if (developmentFixture === "profiles") {
        const fixture = developmentProfileFixture.profiles[0];
        return {
          name: fixture.name,
          travelMm: { ...fixture.travelMm },
          maxJogDistanceMm: fixture.maxJogDistanceMm,
          spindleControl: fixture.spindleControl,
          floodCoolantControl: fixture.floodCoolantControl,
          mistCoolantControl: fixture.mistCoolantControl,
          homingInstalled: fixture.homingInstalled,
          limitSwitchesInstalled: fixture.limitSwitchesInstalled,
          probeInstalled: fixture.probeInstalled,
          probeSettings: fixture.probeSettings,
          emergencyStopInstalled: fixture.emergencyStopInstalled,
          connection: fixture.connection ? { ...fixture.connection } : undefined,
          detectedController: fixture.detectedController
            ? { ...fixture.detectedController }
            : undefined,
        };
      }
      throw new Error("GRBL detection requires the desktop runtime");
    }
    return detectMachineProfile(selectedTransport.id, baudRate);
  };

  return (
    <div className="app-shell">
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

      <main className="workspace">
        <section
          className={`machine-panel is-${workbenchView}`}
          aria-labelledby="machine-state-title"
        >
          <div className="section-heading">
            <div>
              <span>Контроллер GRBL</span>
              <h1 id="machine-state-title">
                {snapshot.machine.reportedMode}
                {snapshot.machine.substate !== undefined
                  ? `:${snapshot.machine.substate}`
                  : ""}
              </h1>
            </div>
            <div className="machine-heading-status">
              {snapshot.alarm ? (
                <div className="operator-notice alarm-notice" role="alert">
                  <div>
                    <span>Аварийное состояние</span>
                    <strong>
                      {snapshot.alarm.code !== undefined
                        ? `ALARM:${snapshot.alarm.code}`
                        : snapshot.alarm.message}
                    </strong>
                  </div>
                  <button
                    aria-label="Разблокировать станок"
                    disabled={controlsBusy || !desktopRuntime}
                    onClick={() => void runAction(unlockAlarm)}
                    title="Разблокировать станок"
                    type="button"
                  >
                    <KeyRound aria-hidden="true" size={13} />
                  </button>
                </div>
              ) : snapshot.resetNotice ? (
                <div className="operator-notice reset-notice" role="status">
                  <div>
                    <span>Контроллер перезапущен</span>
                    <strong>{snapshot.resetNotice.banner}</strong>
                  </div>
                  <button type="button" onClick={() => void runAction(acknowledgeReset)}>
                    OK
                  </button>
                </div>
              ) : (
                <div aria-hidden="true" className="operator-notice is-empty" />
              )}
              <div className="machine-indicators">
                <ProbeIndicator
                  active={snapshot.machine.pins?.probe ?? false}
                  connection={snapshot.connection}
                  onClick={() => setZProbeOpen(true)}
                />
                <span className={`mode-indicator is-${snapshot.machine.mode}`}>
                  {snapshot.machine.mode}
                </span>
              </div>
            </div>
          </div>

          <div className="readout-section">
            <div className="readout-label">
              <span>Рабочая позиция</span>
              <small>{workPositionView.coordinateSystem}</small>
            </div>
            <PositionReadout position={workPositionView.position} />
            <div className="machine-position-secondary">
              <span>Станок · G53</span>
              <code>
                X {formatCoordinate(snapshot.machine.machinePosition?.x)} · Y{" "}
                {formatCoordinate(snapshot.machine.machinePosition?.y)} · Z{" "}
                {formatCoordinate(snapshot.machine.machinePosition?.z)}
              </code>
            </div>
          </div>

          <div className="workbench-tabs" role="tablist" aria-label="Рабочий раздел">
            <button
              aria-controls="program-workbench"
              aria-selected={workbenchView === "program"}
              onClick={() => setWorkbenchView("program")}
              role="tab"
              type="button"
            >
              Задание
            </button>
            <button
              aria-controls="controller-workbench"
              aria-selected={workbenchView === "controller"}
              onClick={() => setWorkbenchView("controller")}
              role="tab"
              type="button"
            >
              Контроллер
            </button>
          </div>

          <div
            className="workbench-panel"
            hidden={workbenchView !== "program"}
            id="program-workbench"
            role="tabpanel"
          >
            <ProgramWorkspace
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
              initialSource={
                developmentPreviewFixture?.lines.map((line) => line.source).join("\n")
              }
              initialToolAssignments={
                developmentFirstCutFixture ? developmentToolAssignments : undefined
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
                machineName: selectedMachine?.name ?? displayedTransport.label,
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
              realRunTarget={
                developmentPreflightFixture || desktopRuntime
              }
              tools={toolLibrary.tools}
            />
          </div>

          <div
            className="workbench-panel"
            hidden={workbenchView !== "controller"}
            id="controller-workbench"
            role="tabpanel"
          >
            <ControllerInspector
              busy={controlsBusy}
              connected={isConnected}
              inspecting={inspecting}
              inspection={inspection}
              onRead={() => void readDeviceInspection()}
            />
          </div>

          <div className="telemetry-row">
            <div>
              <span>Подача</span>
              <strong>{snapshot.machine.feedRate.toFixed(1)}</strong>
              <small>mm/min</small>
            </div>
            <div>
              <span>Шпиндель</span>
              <strong>{snapshot.machine.spindleSpeed.toFixed(0)}</strong>
              <small>rpm</small>
            </div>
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
            onRefreshStatus: () => void runAction(refreshStatus),
            onRefreshTransports: () => void discoverTransports(),
            onTransport: setSelectedTransportId,
          }}
          controls={
            <SafetyControls
              desktopRuntime={desktopRuntime || developmentFixture === "machine-control"}
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
              floodCoolantControl={selectedMachine?.floodCoolantControl ?? false}
              mistCoolantControl={selectedMachine?.mistCoolantControl ?? false}
              activeCoordinateSystem={workPositionView.coordinateSystem.toLowerCase() as WorkCoordinateSystem}
              rotaryAxis={selectedMachine?.rotaryAxis}
            />
          }
          view={{
            baudRate,
            canDisconnect,
            controlsBusy,
            desktopRuntime,
            discovering,
            displayedError,
            displayedTransport,
            hasConnection,
            isConnected,
            likelyGrblOnly,
            selectedMachineName: selectedMachine?.name,
            selectedTransport,
            snapshot,
            transportLocked,
            visibleTransports,
          }}
        />
      </main>

      <Suspense fallback={null}>
        {settingsOpen && (
          <MachineSettingsDialog
            initialQuery={settingsFocus === "motion" ? "acceleration" : ""}
            initialView={settingsFocus === "motion" ? "controller" : "local"}
            onClose={() => setSettingsOpen(false)}
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
            desktopRuntime={desktopRuntime || developmentFixture === "heightmap"}
            initialSnapshot={developmentFixture === "logs" ? developmentAuditFixture : undefined}
            onClose={() => setLogOpen(false)}
            onError={setUiError}
            open
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
            activeCoordinateSystem={workPositionView.coordinateSystem.toLowerCase() as WorkCoordinateSystem}
            desktopRuntime={desktopRuntime || developmentFixture === "heightmap"}
            disabled={controlsBusy}
            gateway={tauriZProbeGateway}
            heightmapGateway={developmentFixture === "heightmap" ? previewHeightmapGateway : tauriHeightmapGateway}
            machineTravel={selectedMachine?.travelMm}
            onAbort={async () => {
              await feedHold();
              const challenge = await requestSoftReset();
              return confirmSoftReset(challenge.id);
            }}
            onClose={() => setZProbeOpen(false)}
            onError={setUiError}
            onSaveSettings={async (settings) => {
              if (!selectedMachine) throw new Error("Сначала выберите профиль станка");
              if (developmentFixture === "heightmap" && !desktopRuntime) {
                setMachineProfiles((current) => ({
                  ...current,
                  profiles: current.profiles.map((profile) => profile.id === selectedMachine.id
                    ? { ...profile, probeInstalled: true, probeSettings: settings }
                    : profile),
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
