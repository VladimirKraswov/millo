import { useEffect, useMemo, useState, useSyncExternalStore } from "react";
import {
  connectTransport,
  disconnect,
  getActiveTransport,
  getControllerSettings,
  inspectDevice,
  isDesktopRuntime,
  listTransports,
  rollbackControllerSetting,
  updateControllerSetting,
} from "../api/controller";
import {
  getApplicationPreferences,
  updateApplicationPreferences,
} from "../api/preferences";
import {
  createMachineProfile,
  detectMachineProfile,
  getMachineProfiles,
  selectMachineProfile,
  updateMachineLocalSettings,
} from "../api/profiles";
import { previewFixtureScriptPlugins } from "../features/script-plugins/previewFixtureScriptPlugins";
import { previewToolLibraryGateway } from "../features/tool-library/previewToolLibraryGateway";
import { useProbeDatum } from "../features/work-zero/useProbeDatum";
import { resolveWorkPosition } from "../features/work-zero/workPositionModel";
import { tauriImageJobGateway } from "../platform/jobs/tauriImageJobGateway";
import { bindMachineStateStream } from "../platform/machine/MachineStateEventStream";
import { tauriHeightmapGateway } from "../platform/machine/tauriHeightmapGateway";
import { tauriMachineCommandGateway } from "../platform/machine/tauriMachineCommandGateway";
import { tauriMachineStateEventStream } from "../platform/machine/tauriMachineStateEventStream";
import { tauriWorkCoordinateGateway } from "../platform/machine/tauriWorkCoordinateGateway";
import { CapabilityGrantStore } from "../platform/plugins/CapabilityGrantStore";
import { tauriScriptPluginGateway } from "../platform/plugins/tauriScriptPluginGateway";
import { tauriToolLibraryGateway } from "../platform/tooling/tauriToolLibraryGateway";
import {
  createImageToGcodePlugin,
  IMAGE_TO_GCODE_PLUGIN_ID,
} from "../plugins/image-to-gcode/createImageToGcodePlugin";
import {
  createPcbFabricationPlugin,
  PCB_FABRICATION_PLUGIN_ID,
} from "../plugins/pcb-fabrication/createPcbFabricationPlugin";
import {
  createSpoilboardSurfacingPlugin,
  SPOILBOARD_SURFACING_PLUGIN_ID,
} from "../plugins/spoilboard-surfacing/createSpoilboardSurfacingPlugin";
import {
  hasControllerSession,
  isControllerConnected,
} from "../shared/controllerReadiness";
import {
  emptySnapshot,
  type ControllerSnapshot,
  type HardwareInspection,
  type TransportDescriptor,
} from "../shared/machine";
import {
  defaultApplicationPreferences,
  type ApplicationPreferencesUpdate,
} from "../shared/preferences";
import type {
  MachineProfile,
  MachineProfileDraft,
  MachineProfileState,
} from "../shared/profile";
import { selectedMachineProfile } from "../shared/profile";
import type { GcodeProgram } from "../shared/program";
import type { InstalledScriptPlugin } from "../shared/scriptPlugins";
import type {
  ControllerSettingEditRequest,
  ControllerSettingsState,
} from "../shared/settings";
import { emptyToolLibrary } from "../shared/tooling";
import { bootstrapPluginHost } from "./bootstrapPluginHost";
import { DeferredDisposal } from "./DeferredDisposal";
import {
  developmentFixture,
  developmentJogSnapshot,
  developmentMachineFixture,
  developmentProbeFixture,
  developmentProfileFixture,
  developmentSettingsFixture,
} from "./developmentFixtures";
import { previewPcbImageJobGateway } from "./previewPcbImageJobGateway";
import {
  createQuickSketchPlugin,
  QUICK_SKETCH_PLUGIN_ID,
} from "../plugins/quick-sketch/createQuickSketchPlugin";
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

export function useWorkstation() {
  const pluginHost = useMemo(
    () =>
      bootstrapPluginHost({
        initialSnapshot: developmentMachineFixture
          ? developmentJogSnapshot
          : emptySnapshot,
        machineCommands: tauriMachineCommandGateway,
        workCoordinates: tauriWorkCoordinateGateway,
        imageJobs:
          developmentFixture === "pcb"
            ? previewPcbImageJobGateway
            : tauriImageJobGateway,
        toolLibrary: isDesktopRuntime()
          ? tauriToolLibraryGateway
          : previewToolLibraryGateway,
        grants: new CapabilityGrantStore([
          {
            pluginId: QUICK_SKETCH_PLUGIN_ID,
            capabilities: ["ui.contribute", "jobs.create", "tools.read"],
          },
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
          createQuickSketchPlugin({ initialOpen: developmentFixture === "sketch" }),
          createImageToGcodePlugin({
            initialOpen: developmentFixture === "image-job",
          }),
          createPcbFabricationPlugin({
            initialOpen: developmentFixture === "pcb",
          }),
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
  const [activeTransport, setActiveTransport] = useState<TransportDescriptor>(
    disconnectedTransport,
  );
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
  const [controllerSettings, setControllerSettings] = useState<
    ControllerSettingsState | undefined
  >(
    developmentFixture === "settings" || developmentMachineFixture
      ? developmentSettingsFixture
      : undefined,
  );
  const [onboardingDraft, setOnboardingDraft] = useState<MachineProfileDraft>();
  const [activeProgram, setActiveProgram] = useState<GcodeProgram>();
  const [settingsOpen, setSettingsOpen] = useState(
    developmentFixture === "settings",
  );
  const [settingsFocus, setSettingsFocus] = useState<"local" | "motion">(
    "local",
  );
  const [inspecting, setInspecting] = useState(false);
  const [busy, setBusy] = useState(false);
  const [discovering, setDiscovering] = useState(false);
  const [uiError, setUiError] = useState<string>();
  const [helpOpen, setHelpOpen] = useState(false);
  const [noticeError, setNoticeError] = useState<string>();
  const [logOpen, setLogOpen] = useState(developmentFixture === "logs");
  const [consoleOpen, setConsoleOpen] = useState(
    developmentFixture === "console",
  );
  const [applicationPreferences, setApplicationPreferences] = useState(
    defaultApplicationPreferences,
  );
  const [workZeroOpen, setWorkZeroOpen] = useState(false);
  const [zProbeOpen, setZProbeOpen] = useState(developmentProbeFixture);
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
    () =>
      new DeferredDisposal(
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
    void getApplicationPreferences()
      .then(setApplicationPreferences)
      .catch((error: unknown) => setUiError(String(error)));
    void tauriScriptPluginGateway
      .list()
      .then(setScriptPlugins)
      .catch((error: unknown) => setUiError(String(error)));
  }, [desktopRuntime]);

  const saveApplicationPreferences = async (
    update: ApplicationPreferencesUpdate,
  ) => {
    if (!desktopRuntime) {
      const next = { ...applicationPreferences, ...update };
      setApplicationPreferences(next);
      return next;
    }
    const next = await updateApplicationPreferences(update);
    setApplicationPreferences(next);
    return next;
  };

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
      setMachineProfiles(
        settings.profileId
          ? { ...profiles, selectedProfileId: settings.profileId }
          : profiles,
      );
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
    )
      return;
    void synchronizeConnectedMachine();
  }, [
    controllerSettings,
    desktopRuntime,
    machineSyncAttempted,
    machineSyncing,
    snapshot.connection,
  ]);

  useEffect(() => {
    if (snapshot.connection === "disconnected") {
      setMachineSyncAttempted(false);
      setMachineSyncing(false);
    }
  }, [snapshot.connection]);

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
    snapshot.connection !== "disconnected" &&
    snapshot.connection !== "connecting";
  const transportLocked = hasConnection || snapshot.connection === "connecting";
  const visibleTransports = transports.filter(
    (transport) =>
      !likelyGrblOnly ||
      transport.likelyGrbl ||
      (transportLocked && transport.id === activeTransport.id),
  );
  const selectedTransport =
    visibleTransports.find(
      (transport) => transport.id === selectedTransportId,
    ) ??
    visibleTransports[0] ??
    activeTransport;
  const displayedTransport = transportLocked
    ? activeTransport
    : selectedTransport;
  const displayedError = uiError ?? snapshot.lastError;
  useEffect(() => {
    if (displayedError) setNoticeError(displayedError);
  }, [displayedError]);
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
        controllerSettings?.snapshot.values.find(
          (setting) => setting.key === key,
        )?.value,
      ),
    )
    .filter((value) => Number.isFinite(value) && value >= 10);
  const maxJogFeedMmPerMin =
    jogAxisRates.length > 0 ? Math.min(...jogAxisRates) : 1_000;
  const maxJogDistanceMm = selectedMachine?.maxJogDistanceMm ?? 50;
  const workPositionView = resolveWorkPosition(snapshot, inspection);

  const {
    datum: probeEstablishedZDatum,
    remember: rememberProbeEstablishedZDatum,
  } = useProbeDatum({
    snapshot,
    coordinateSystem: workPositionView.coordinateSystem,
    profileId: selectedMachine?.id,
    gateway: desktopRuntime ? tauriHeightmapGateway : undefined,
  });

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
          connection: fixture.connection
            ? { ...fixture.connection }
            : undefined,
          detectedController: fixture.detectedController
            ? { ...fixture.detectedController }
            : undefined,
        };
      }
      throw new Error("GRBL detection requires the desktop runtime");
    }
    return detectMachineProfile(selectedTransport.id, baudRate);
  };
  return {
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
  };
}
