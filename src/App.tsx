import { useEffect, useMemo, useRef, useState, useSyncExternalStore } from "react";
import {
  ChevronDown,
  KeyRound,
  PlugZap,
  Puzzle,
  RefreshCw,
  ScrollText,
  Unplug,
} from "lucide-react";

import { bootstrapPluginHost } from "./app/bootstrapPluginHost";
import { DeferredDisposal } from "./app/DeferredDisposal";
import { CapabilityGrantStore } from "./platform/plugins/CapabilityGrantStore";
import { createImageToGcodePlugin, IMAGE_TO_GCODE_PLUGIN_ID } from "./plugins/image-to-gcode/createImageToGcodePlugin";
import {
  createSpoilboardSurfacingPlugin,
  SPOILBOARD_SURFACING_PLUGIN_ID,
} from "./plugins/spoilboard-surfacing/createSpoilboardSurfacingPlugin";
import {
  acknowledgeReset,
  clearMockAlarm,
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
  triggerMockAlarm,
  triggerMockDisconnect,
  triggerMockRun,
  triggerMockReset,
  triggerMockTimeout,
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
import { ReadinessPanel } from "./components/ReadinessPanel";
import { SafetyControls } from "./components/SafetyControls";
import { previewFixtureProgram } from "./features/program/previewFixtureProgram";
import { previewFixtureAirSquareProgram } from "./features/program/previewFixtureAirSquare";
import { previewFixturePreflightGateway } from "./features/program/previewFixturePreflight";
import {
  previewFixtureCheckCompleteSender,
  previewFixtureCompletedSender,
  previewFixtureCheckControlGateway,
  previewFixtureCheckRunningSender,
  previewFixtureFirstCutGateway,
  previewFixtureFirstCutProgram,
  previewFixtureProgramGateway,
  previewFixtureRecoveryGateway,
  previewFixtureToolChangeSender,
} from "./features/program/previewFixtureFirstCut";
import { ProgramWorkspace } from "./features/program/ProgramWorkspace";
import { MachineProfiles } from "./features/machine-profiles/MachineProfiles";
import { MachineSettingsDialog } from "./features/machine-settings/MachineSettingsDialog";
import { DiagnosticLogViewer } from "./features/diagnostics/DiagnosticLogViewer";
import { ProbeIndicator } from "./features/probe/ProbeIndicator";
import { ZProbeDialog } from "./features/probe/ZProbeDialog";
import { previewHeightmapGateway } from "./features/heightmap/previewHeightmapGateway";
import { heightmapHasCurrentZDatum } from "./features/heightmap/heightmapModel";
import { WorkZeroDialog } from "./features/work-zero/WorkZeroDialog";
import { ToolLibraryDialog } from "./features/tool-library/ToolLibraryDialog";
import { WorkspaceToolsMenu } from "./components/WorkspaceToolsMenu";
import { ScriptPluginContributions } from "./features/script-plugins/ScriptPluginContributions";
import { ScriptPluginManager } from "./features/script-plugins/ScriptPluginManager";
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
import { tauriDryRunGateway } from "./platform/program/tauriDryRunGateway";
import { tauriRealRunPreflightGateway } from "./platform/program/tauriRealRunPreflightGateway";
import {
  emptySnapshot,
  type ControllerSnapshot,
  type HardwareInspection,
  type Position,
  type TransportDescriptor,
  type WorkCoordinateSystem,
  type ZProbeOutcome,
} from "./shared/machine";
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
import type { AuditLogSnapshot } from "./shared/audit";
import type { InstalledScriptPlugin } from "./shared/scriptPlugins";

const connectionLabels = {
  disconnected: "Отключено",
  connecting: "Подключение",
  connected: "Подключено",
  recovering: "Восстановление",
  faulted: "Ошибка",
} as const;

const mockTransport: TransportDescriptor = {
  id: "mock",
  kind: "mock",
  label: "Mock GRBL",
  detail: "Встроенный тестовый контроллер",
  likelyGrbl: true,
  matchReason: "Встроенный тестовый контроллер",
};

interface ProbeEstablishedZDatum {
  readonly coordinateSystem: WorkCoordinateSystem;
  readonly resetCount: number;
  readonly reconnectCount: number;
  readonly source: "probe" | "heightmap";
  readonly workCoordinateOffsetZ?: number;
}

const baudRates = [9_600, 19_200, 38_400, 57_600, 115_200, 230_400];
const developmentFixture = import.meta.env.DEV
  ? new URLSearchParams(window.location.search).get("fixture")
  : undefined;
const developmentPreviewFixture =
  developmentFixture === "air-square"
    ? previewFixtureAirSquareProgram
    : developmentFixture === "first-cut" ||
        developmentFixture === "check-complete" ||
        developmentFixture === "check-running" ||
        developmentFixture === "run-complete" ||
        developmentFixture === "tool-change" ||
        developmentFixture === "recovery"
      ? previewFixtureFirstCutProgram
      : developmentFixture === "program" || developmentFixture === "preflight" || developmentFixture === "heightmap"
        ? previewFixtureProgram
        : undefined;
const developmentPreflightFixture =
  developmentFixture === "preflight" ||
  developmentFixture === "heightmap" ||
  developmentFixture === "first-cut" ||
  developmentFixture === "check-complete" ||
  developmentFixture === "check-running" ||
  developmentFixture === "run-complete" ||
  developmentFixture === "tool-change" ||
  developmentFixture === "recovery" ||
  developmentFixture === "air-square";
const developmentFirstCutFixture =
  developmentFixture === "first-cut" ||
  developmentFixture === "check-complete" ||
  developmentFixture === "check-running" ||
  developmentFixture === "run-complete" ||
  developmentFixture === "tool-change" ||
  developmentFixture === "recovery" ||
  developmentFixture === "air-square";
const developmentJogFixture = ["jog", "jog-active", "alarm", "reset", "logs"].includes(
  developmentFixture ?? "",
);
const developmentProbeFixture = developmentFixture === "probe" || developmentFixture === "heightmap";
const developmentMachineFixture =
  developmentJogFixture || developmentProbeFixture || developmentPreflightFixture;
const developmentMachineMode =
  developmentFixture === "jog-active"
    ? "jog"
    : developmentFixture === "alarm"
      ? "alarm"
      : "idle";
const developmentJogSnapshot: ControllerSnapshot = {
  ...emptySnapshot,
  connection: "connected",
  machine: {
    ...emptySnapshot.machine,
    mode: developmentMachineMode,
    reportedMode:
      developmentMachineMode === "jog"
        ? "Jog"
        : developmentMachineMode === "alarm"
          ? "Alarm"
          : "Idle",
    machinePosition: { x: 152.4, y: 91.2, z: -4.75 },
    workPosition: { x: 12.4, y: 8.2, z: 5.25 },
    workCoordinateOffset: { x: 140, y: 83, z: -10 },
    feedRate: 0,
    spindleSpeed: 0,
    pins: developmentProbeFixture
      ? {
          raw: "P",
          xLimit: false,
          yLimit: false,
          zLimit: false,
          aLimit: false,
          bLimit: false,
          cLimit: false,
          probe: developmentFixture === "probe",
          door: false,
          hold: false,
          softReset: false,
          cycleStart: false,
        }
      : undefined,
  },
  pollSequence: 42,
  pollIntervalMs: 250,
  statusTimeoutMs: 500,
  failureThreshold: 2,
  alarm:
    developmentFixture === "alarm"
      ? { code: 3, message: "Reset while in motion" }
      : undefined,
  resetNotice:
    developmentFixture === "reset"
      ? { banner: "Grbl 1.1f ['$' for help]", version: "1.1f", sequence: 4 }
      : undefined,
};
const developmentProfileFixture: MachineProfileState = {
  profiles: [
    {
      id: "machine-0001",
      name: "LUNYEE CNC",
      travelMm: { x: 500, y: 500, z: 200 },
      maxJogDistanceMm: 50,
      spindleControl: "manual",
      homingInstalled: false,
      limitSwitchesInstalled: false,
      probeInstalled: developmentProbeFixture,
      probeSettings: {
        mode: developmentFixture === "heightmap" ? "heightmap" : "workZero",
        plateThicknessMm: 19.1,
        maxTravelMm: 10,
        probeFeedMmPerMin: 25,
        retractMm: 3,
        retractFeedMmPerMin: 100,
      },
      emergencyStopInstalled: false,
      connection: { transportId: "mock", baudRate: 115_200 },
      detectedController: { firmwareVersion: "1.1f.20230316" },
    },
  ],
  selectedProfileId: "machine-0001",
};
const developmentSettingsFixture: ControllerSettingsState = {
  snapshot: {
    revision: 4,
    firmwareVersion: "1.1f.20230316",
    firmwareBuildInfo: "LUNYEE_4axis_Control",
    values: [
      { key: "$21", value: "0", title: "Hard limits", group: "safety", kind: "boolean", known: true },
      { key: "$22", value: "0", title: "Homing cycle", group: "homing", kind: "boolean", known: true },
      { key: "$100", value: "1600.000", title: "X steps per millimeter", group: "calibration", kind: "decimal", unit: "step/mm", known: true },
      { key: "$110", value: "1000.000", title: "X maximum rate", group: "motion", kind: "decimal", unit: "mm/min", known: true },
      { key: "$120", value: "600.000", title: "X acceleration", group: "motion", kind: "decimal", unit: "mm/s^2", known: true },
      { key: "$130", value: "500.000", title: "X maximum travel", group: "travel", kind: "decimal", unit: "mm", known: true },
      { key: "$131", value: "500.000", title: "Y maximum travel", group: "travel", kind: "decimal", unit: "mm", known: true },
      { key: "$132", value: "200.000", title: "Z maximum travel", group: "travel", kind: "decimal", unit: "mm", known: true },
      { key: "$200", value: "7.5", title: "Firmware setting 200", group: "advanced", kind: "decimal", known: false },
    ],
  },
  sessionBaseline: {
    "$21": "0",
    "$22": "0",
    "$100": "1600.000",
    "$110": "1000.000",
    "$120": "500.000",
    "$130": "500.000",
    "$131": "500.000",
    "$132": "200.000",
    "$200": "7.5",
  },
  previousBaseline: { "$120": "400.000" },
  revisionCount: 2,
  profileId: "machine-0001",
  fingerprint: {
    key: "port:0483:5740:lunyee_4axis_control:devcuusbmodem11101",
    confidence: "portBound",
    label: "LUNYEE_4axis_Control · 1.1f.20230316 · /dev/cu.usbmodem11101",
  },
};
const developmentAuditFixture: AuditLogSnapshot = {
  sessionId: "preview-2048",
  activePath: "/Users/operator/Library/Application Support/Millo/logs/millo-audit.jsonl",
  droppedEntries: 0,
  writeFailures: 0,
  entries: [
    {
      schemaVersion: 1,
      sequence: 201,
      sessionId: "preview-2048",
      timestampMs: Date.now() - 8_500,
      level: "info",
      category: "transport",
      event: "transport.connect.completed",
      message: "Controller connected and synchronized",
      data: { port: "/dev/cu.usbmodem11101", firmware: "Grbl 1.1f" },
    },
    {
      schemaVersion: 1,
      sequence: 202,
      sessionId: "preview-2048",
      timestampMs: Date.now() - 5_200,
      level: "warning",
      category: "program",
      event: "program.preflight.report",
      message: "Program preflight is blocked",
      data: { sourceName: "millo-solar-guilloche.nc", blocker: "Work zero not verified" },
    },
    {
      schemaVersion: 1,
      sequence: 203,
      sessionId: "preview-2048",
      timestampMs: Date.now() - 2_100,
      level: "error",
      category: "sender",
      event: "sender.snapshot",
      message: "ALARM:2 at source line 18",
      data: { sourceLine: 18, command: "G1 Z-0.200 F80", state: "failed" },
    },
  ],
};

const formatCoordinate = (value: number | undefined): string =>
  value === undefined ? "--" : value.toFixed(3);

function AxisReadout({ axis, value }: { axis: string; value: number | undefined }) {
  return (
    <div className="axis-readout">
      <span>{axis}</span>
      <strong>{formatCoordinate(value)}</strong>
      <small>mm</small>
    </div>
  );
}

function PositionReadout({ position }: { position?: Position }) {
  return (
    <div className="position-grid">
      <AxisReadout axis="X" value={position?.x} />
      <AxisReadout axis="Y" value={position?.y} />
      <AxisReadout axis="Z" value={position?.z} />
      {position?.a !== undefined && <AxisReadout axis="A" value={position.a} />}
    </div>
  );
}

export default function App() {
  const pluginHost = useMemo(
    () =>
      bootstrapPluginHost({
        initialSnapshot: developmentMachineFixture ? developmentJogSnapshot : emptySnapshot,
        machineCommands: tauriMachineCommandGateway,
        workCoordinates: tauriWorkCoordinateGateway,
        imageJobs: tauriImageJobGateway,
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
        ]),
        bundledPlugins: [
          createImageToGcodePlugin({ initialOpen: developmentFixture === "image-job" }),
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
  const [transports, setTransports] = useState<TransportDescriptor[]>([
    mockTransport,
  ]);
  const [selectedTransportId, setSelectedTransportId] = useState("mock");
  const [activeTransport, setActiveTransport] =
    useState<TransportDescriptor>(mockTransport);
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

  const runMockAction = async (action: () => Promise<void>) => {
    setUiError(undefined);
    try {
      await action();
    } catch (error) {
      setUiError(String(error));
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

  const isConnected = snapshot.connection === "connected";
  const hasConnection =
    snapshot.connection === "connected" || snapshot.connection === "recovering";
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
    activeTransport.kind === "mock" || (
      controllerSettings?.profileId !== undefined &&
      selectedMachine?.id === controllerSettings.profileId
    );
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
      spindleControl: profile.spindleControl,
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
              dryRunAvailable={
                desktopRuntime &&
                activeTransport.kind === "mock" &&
                snapshot.connection === "connected" &&
                snapshot.machine.mode === "idle" &&
                snapshot.alarm === undefined &&
                snapshot.resetNotice === undefined
              }
              dryRunGateway={
                developmentFixture === "check-running"
                  ? previewFixtureCheckControlGateway
                  : desktopRuntime
                    ? tauriDryRunGateway
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
              incomingJob={generatedJob}
              initialRunIntent={
                developmentFixture === "check-complete" ||
                developmentFixture === "run-complete"
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
                  activeTransport.kind === "serial" &&
                  snapshot.connection === "connected" &&
                  snapshot.machine.mode === "idle" &&
                  snapshot.alarm === undefined &&
                  snapshot.resetNotice === undefined &&
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
                developmentPreflightFixture ||
                activeTransport.kind === "serial" ||
                selectedTransport.kind === "serial"
              }
            />
          </div>

          <div
            className="workbench-panel"
            hidden={workbenchView !== "controller"}
            id="controller-workbench"
            role="tabpanel"
          >
            <section className="device-inspector" aria-labelledby="inspector-title">
            <div className="inspector-heading">
              <div>
                <span>Только чтение</span>
                <h2 id="inspector-title">Состояние контроллера</h2>
              </div>
              <button
                disabled={!isConnected || controlsBusy}
                onClick={() => void readDeviceInspection()}
                type="button"
              >
                <span>{inspecting ? "Чтение" : "Считать"}</span>
                <code>$I · $$ · $G · $#</code>
              </button>
            </div>

            {inspection ? (
              <>
                <ReadinessPanel report={inspection.readiness} />
                <details className="technical-inspection">
                  <summary>Технические данные контроллера</summary>
                  <div className="inspector-content">
                    <div className="inspector-identity">
                      <div className="firmware-readout">
                        <span>Прошивка</span>
                        <strong>
                          {inspection.device.firmwareVersion ?? "Неизвестная версия GRBL"}
                        </strong>
                        <small>
                          {inspection.device.firmwareBuildInfo ?? "Нет сведений о сборке"}
                        </small>
                      </div>
                      <dl className="inspection-meta">
                        <div>
                          <dt>Возможности</dt>
                          <dd title={inspection.device.firmwareOptions}>
                            {inspection.device.controllerCapabilities
                              ? `${inspection.device.controllerCapabilities.optionFlags} · P${inspection.device.controllerCapabilities.plannerBufferBlocks ?? "?"} · RX${inspection.device.controllerCapabilities.rxBufferBytes ?? "?"}`
                              : (inspection.device.firmwareOptions ?? "--")}
                          </dd>
                        </div>
                        <div>
                          <dt>Настройки</dt>
                          <dd>{Object.keys(inspection.device.settings).length}</dd>
                        </div>
                        <div>
                          <dt>Параметры</dt>
                          <dd>
                            {Object.keys(inspection.device.parameters).length}
                          </dd>
                        </div>
                      </dl>
                      <div className="modal-state">
                        <span>Модальное состояние</span>
                        <div>
                          {inspection.device.modalState.map((mode) => (
                            <code key={mode}>{mode}</code>
                          ))}
                        </div>
                      </div>
                      <div
                        className="query-results"
                        aria-label="Результаты запросов к контроллеру"
                      >
                        {inspection.device.responses.map((response) => (
                          <div
                            className={`is-${response.completion}`}
                            key={response.command}
                          >
                            <code>{response.command}</code>
                            <strong>
                              {response.completion}
                              {response.code !== undefined
                                ? `:${response.code}`
                                : ""}
                            </strong>
                          </div>
                        ))}
                      </div>
                    </div>

                    <div className="inspector-registers">
                      <div>
                        <span>Настройки контроллера</span>
                        <div className="register-list">
                          {Object.entries(inspection.device.settings).map(
                            ([key, value]) => (
                              <div key={key}>
                                <code>{key}</code>
                                <strong>{value}</strong>
                              </div>
                            ),
                          )}
                        </div>
                      </div>
                      <div>
                        <span>Системы координат</span>
                        <div className="register-list">
                          {Object.entries(inspection.device.parameters).map(
                            ([key, value]) => (
                              <div key={key}>
                                <code>{key}</code>
                                <strong>{value}</strong>
                              </div>
                            ),
                          )}
                        </div>
                      </div>
                    </div>
                  </div>
                </details>
              </>
            ) : (
              <div className="inspector-empty">
                <strong>Профиль контроллера не считан</strong>
                <span>Движение и управление шпинделем недоступны</span>
              </div>
            )}
            </section>
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

        <aside className="control-panel" aria-label="Управление подключением">
          <div className="panel-title">
            <span>Подключение</span>
            <strong>{displayedTransport.label}</strong>
            <small>
              {selectedMachine
                ? `Станок: ${selectedMachine.name}`
                : "Сначала добавьте или выберите станок"}
            </small>
          </div>

          <div className="connection-primary">
            {hasConnection ? (
              <>
                <div className={`connection-inline is-${snapshot.connection}`}>
                  <i aria-hidden="true" />
                  <span>{connectionLabels[snapshot.connection]}</span>
                </div>
                <button
                  aria-label="Отключить"
                  className="disconnect-action"
                  disabled={controlsBusy || !canDisconnect}
                  onClick={() => void disconnectController()}
                  title="Отключить"
                  type="button"
                >
                  <Unplug aria-hidden="true" size={15} />
                </button>
              </>
            ) : (
              <button
                className="primary-action"
                disabled={controlsBusy || !desktopRuntime}
                onClick={() => void connectSelectedTransport()}
                type="button"
              >
                <PlugZap aria-hidden="true" size={15} />
                Подключить
              </button>
            )}
          </div>

          <button className="log-open-action" onClick={() => setLogOpen(true)} type="button">
            <ScrollText aria-hidden="true" size={14} />
            <span>
              <strong>Журнал событий</strong>
              <small>Подключение · GRBL · Выполнение</small>
            </span>
          </button>

          {hasConnection && (
            <SafetyControls
              desktopRuntime={desktopRuntime}
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
            />
          )}

          <details className="control-disclosure" open={!hasConnection}>
            <summary>
              <span>Параметры подключения</span>
              <ChevronDown aria-hidden="true" size={14} />
            </summary>
            <div className="transport-config">
              <label className="transport-filter">
                <input
                  checked={likelyGrblOnly}
                  disabled={controlsBusy || discovering}
                  onChange={(event) => setLikelyGrblOnly(event.target.checked)}
                  type="checkbox"
                />
                <span>Только вероятные GRBL</span>
              </label>
              <label htmlFor="transport-select">Устройство</label>
              <div className="transport-select-row">
                <select
                  id="transport-select"
                  disabled={
                    transportLocked || controlsBusy || discovering || !desktopRuntime
                  }
                  onChange={(event) => setSelectedTransportId(event.target.value)}
                  value={selectedTransport.id}
                >
                  {visibleTransports.map((transport) => (
                    <option key={transport.id} value={transport.id}>
                      {transport.label}
                    </option>
                  ))}
                </select>
                <button
                  aria-label="Обновить список портов"
                  disabled={
                    transportLocked || controlsBusy || discovering || !desktopRuntime
                  }
                  onClick={() => void discoverTransports()}
                  title="Обновить список портов"
                  type="button"
                >
                  <RefreshCw aria-hidden="true" size={15} />
                </button>
              </div>
              <small>
                {selectedTransport.detail}
                {selectedTransport.kind === "serial" &&
                  selectedTransport.matchReason &&
                  ` · ${selectedTransport.matchReason}`}
              </small>

              {selectedTransport.kind === "serial" && (
                <label className="baud-field" htmlFor="baud-rate">
                  <span>Скорость порта</span>
                  <select
                    id="baud-rate"
                    disabled={transportLocked || controlsBusy}
                    onChange={(event) => setBaudRate(Number(event.target.value))}
                    value={baudRate}
                  >
                    {baudRates.map((rate) => (
                      <option key={rate} value={rate}>
                        {rate.toLocaleString("en-US", { useGrouping: false })}
                      </option>
                    ))}
                  </select>
                </label>
              )}
            </div>
          </details>

          {hasConnection && (
            <details className="control-disclosure diagnostics-disclosure">
              <summary>
                <span>Диагностика соединения</span>
                <ChevronDown aria-hidden="true" size={14} />
              </summary>
              <div className="lifecycle-metrics">
                <div>
                  <span>Опрос</span>
                  <strong>{snapshot.pollIntervalMs || "--"} ms</strong>
                </div>
                <div>
                  <span>Тайм-аут</span>
                  <strong>{snapshot.statusTimeoutMs || "--"} ms</strong>
                </div>
                <div>
                  <span>Сбои</span>
                  <strong>
                    {snapshot.consecutiveFailures}/{snapshot.failureThreshold || "--"}
                  </strong>
                </div>
                <div>
                  <span>Переподключения</span>
                  <strong>{snapshot.reconnectCount}</strong>
                </div>
              </div>
              <button
                className="status-request-action"
                disabled={controlsBusy || !isConnected}
                onClick={() => void runAction(refreshStatus)}
                type="button"
              >
                <RefreshCw aria-hidden="true" size={14} />
                Запросить статус
                <kbd>?</kbd>
              </button>

              {displayedTransport.kind === "mock" && (
                <div className="mock-scenarios">
                  <span>Сценарии Mock GRBL</span>
                  <div>
                    <button
                      disabled={controlsBusy || !isConnected}
                      onClick={() => void runMockAction(triggerMockRun)}
                      type="button"
                    >
                      Run state
                    </button>
                    <button
                      disabled={controlsBusy || !isConnected}
                      onClick={() => void runMockAction(triggerMockReset)}
                      type="button"
                    >
                      Reset banner
                    </button>
                    <button
                      disabled={controlsBusy || !isConnected}
                      onClick={() => void runMockAction(() => triggerMockAlarm(3))}
                      type="button"
                    >
                      ALARM:3
                    </button>
                    <button
                      disabled={controlsBusy || !isConnected || !snapshot.alarm}
                      onClick={() => void runMockAction(clearMockAlarm)}
                      type="button"
                    >
                      Clear alarm
                    </button>
                    <button
                      disabled={controlsBusy || !isConnected}
                      onClick={() => void runMockAction(triggerMockTimeout)}
                      type="button"
                    >
                      Timeout ×2
                    </button>
                    <button
                      disabled={controlsBusy || !isConnected}
                      onClick={() => void runMockAction(triggerMockDisconnect)}
                      type="button"
                    >
                      Link drop
                    </button>
                  </div>
                </div>
              )}
            </details>
          )}

          {!desktopRuntime && (
            <p className="runtime-note">Управление доступно в окне Tauri.</p>
          )}
          {displayedError && (
            <div className="error-note" role="alert">
              <span>{displayedError}</span>
              <div>
                <button onClick={() => setLogOpen(true)} type="button">Журнал</button>
                <button aria-label="Закрыть ошибку" onClick={() => setUiError(undefined)} type="button">×</button>
              </div>
            </div>
          )}
        </aside>
      </main>

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
        open={settingsOpen}
        profile={selectedMachine}
        settings={controllerSettings}
      />
      <DiagnosticLogViewer
        desktopRuntime={desktopRuntime || developmentFixture === "heightmap"}
        initialSnapshot={developmentFixture === "logs" ? developmentAuditFixture : undefined}
        onClose={() => setLogOpen(false)}
        onError={setUiError}
        open={logOpen}
      />

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
        open={zProbeOpen}
        profileId={selectedMachine?.id}
        program={activeProgram}
        probeInstalled={selectedMachine?.probeInstalled ?? false}
        settings={selectedMachine?.probeSettings}
        snapshot={snapshot}
      />
      {pluginHost.tools && (
        <ToolLibraryDialog
          onClose={() => setToolLibraryOpen(false)}
          open={toolLibraryOpen}
          service={pluginHost.tools}
        />
      )}
      <ScriptPluginContributions
        gateway={tauriScriptPluginGateway}
        jobs={pluginHost.generatedJobs}
        machine={pluginHost.machineState}
        onError={setUiError}
        plugins={scriptPlugins}
        registry={pluginHost.uiRegistry}
      />
      <ScriptPluginManager
        gateway={tauriScriptPluginGateway}
        onChange={setScriptPlugins}
        onClose={() => setScriptManagerOpen(false)}
        onError={setUiError}
        open={scriptManagerOpen}
        plugins={scriptPlugins}
      />
    </div>
  );
}
