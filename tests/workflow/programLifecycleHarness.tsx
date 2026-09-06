import { StrictMode } from "react";
import { flushSync } from "react-dom";
import { createRoot, type Root } from "react-dom/client";
import { DialogHost } from "../../src/components/DialogSurface";
import { useProgramWorkspace } from "../../src/features/program/useProgramWorkspace";
import type { ProgramWorkspaceProps } from "../../src/features/program/programWorkspaceTypes";
import { ProgramEditor } from "../../src/features/program/ProgramEditor";
import { ToolChangeDialog } from "../../src/features/program/ToolChangeDialog";
import { FirstCutAuthorizationDialog } from "../../src/features/program/FirstCutAuthorizationDialog";
import { ProgramRecoveryDialog } from "../../src/features/program/ProgramRecoveryDialog";
import { ToolpathPreview } from "../../src/features/program/ToolpathPreview";
import { HeightmapPanel } from "../../src/features/heightmap/HeightmapPanel";
import { ZProbeDialog } from "../../src/features/probe/ZProbeDialog";
import { emptyHeightmapOperation, emptySurfaceSession } from "../../src/features/heightmap/heightmapDefaults";
import { previewHeightmapGateway } from "../../src/features/heightmap/previewHeightmapGateway";
import { defaultZProbeSettings } from "../../src/shared/profile";
import { emptySnapshot } from "../../src/shared/machine";
import { idleSenderSnapshot } from "../../src/shared/dryRun";
import type { SenderSnapshot } from "../../src/shared/dryRun";
import type { GcodeProgram } from "../../src/shared/program";
import type { RunPreflightReport } from "../../src/shared/realRun";
import {
  previewFixtureFirstCutProgram as program,
  previewFixtureFirstCutReport as report,
  previewFixtureFirstCutGateway as realGateway,
  previewFixtureRecoveryGateway,
} from "../../src/features/program/previewFixtureFirstCut";

export { program, report, idleSenderSnapshot };
export const deferred = <T,>() => {
  let resolve!: (value: T) => void;
  let reject!: (reason: unknown) => void;
  const promise = new Promise<T>((yes, no) => { resolve = yes; reject = no; });
  return { promise, resolve, reject };
};
let root: Root;
function mount(element: React.ReactNode) {
  root?.unmount();
  const host = document.createElement("div");
  document.body.replaceChildren(host);
  root = createRoot(host);
  render(element);
}
function render(element: React.ReactNode) {
  flushSync(() => root.render(<StrictMode><DialogHost>{element}</DialogHost></StrictMode>));
}
export function unmount() { flushSync(() => root.unmount()); }

export const snapshot = {
  ...emptySnapshot, connection: "connected" as const,
  machine: { ...emptySnapshot.machine, mode: "idle" as const, reportedMode: "Idle" },
};
export let workspace: ReturnType<typeof useProgramWorkspace>;
export let workspaceProps: ProgramWorkspaceProps;
export const parses: ReturnType<typeof deferred<GcodeProgram>>[] = [];
export const preflights: ReturnType<typeof deferred<RunPreflightReport>>[] = [];
export const commands: ReturnType<typeof deferred<SenderSnapshot>>[] = [];
export const inspections: unknown[] = [];
const senderListeners = new Set<(snapshot: SenderSnapshot) => void>();
export function emitSender(next: SenderSnapshot) {
  flushSync(() => { for (const handler of senderListeners) handler(next); });
}
function Workspace() {
  workspace = useProgramWorkspace(workspaceProps);
  return <output aria-label="workspace">{JSON.stringify({
    source: workspace.loaded?.source, sourceName: workspace.program?.sourceName,
    loading: workspace.loading, preflightLoading: workspace.preflightLoading,
    report: workspace.reportForProgram?.sourceName, error: workspace.error,
    checkRunVisible: workspace.checkRunVisible, checkRunAvailable: workspace.checkRunAvailable,
    senderState: workspace.sender.state, recoveryChecked: workspace.recoveryChecked,
    busy: workspace.senderCommandBusy, firstCutOpen: workspace.firstCutOpen,
  })}</output>;
}
export function mountWorkspace() {
  workspaceProps = {
    desktopRuntime: true, initialProgram: program, initialSource: "G1 X1",
    gateway: { parse: () => { const request = deferred<GcodeProgram>(); parses.push(request); return request.promise; } },
    realRunTarget: true, realRunAvailable: true,
    realRunGateway: { ...realGateway,
      preflight: () => { const request = deferred<RunPreflightReport>(); preflights.push(request); return request.promise; },
      startCheck: () => { const request = deferred<SenderSnapshot>(); commands.push(request); return request.promise; },
    },
    senderGateway: { snapshot: async () => idleSenderSnapshot, subscribe: async (handler) => {
      senderListeners.add(handler); return () => { senderListeners.delete(handler); };
    } },
    onInspection: (value) => inspections.push(value),
    machineContext: {
      activeCoordinateSystem: "G54", busy: false, machineBound: true,
      machineName: "Fixture", machineProfileId: "one", machineSyncing: false,
      onAcknowledgeReset: () => {}, onConnect: () => {}, onOpenWorkZero: () => {},
      onReturnToWorkOrigin: async () => {}, onSyncMachine: () => {}, onUnlock: () => {},
      snapshot, workPosition: { x: 0, y: 0, z: 0 },
    },
  };
  mount(<Workspace />);
}
export function updateWorkspace(props: Partial<ProgramWorkspaceProps>) {
  workspaceProps = { ...workspaceProps, ...props }; render(<Workspace />);
}

export const pendingDialog = deferred<any>();
export let dialogClosed = 0;
export let preparedCount = 0;
export let startedCount = 0;
export function mountToolChange() { mount(<ToolChangeDialog {...toolChangeProps(5)} />); }
const toolChangeProps = (sourceLine: number) => ({
  open: true, sourceLine, requestedTool: sourceLine,
  onClose: () => { dialogClosed += 1; }, onComplete: () => pendingDialog.promise,
});
export function nextToolChange() { render(<ToolChangeDialog {...toolChangeProps(6)} />); }
export async function mountRecovery() {
  const candidate = (await previewFixtureRecoveryGateway.recoveryCandidate())!;
  mount(<ProgramRecoveryDialog candidate={candidate} open onClose={() => { dialogClosed += 1; }}
    onPrepare={() => pendingDialog.promise} onPrepared={() => { preparedCount += 1; }} onDismiss={async () => {}} />);
}
export function mountFirstCut(open = true) {
  const element = <FirstCutAuthorizationDialog open={open} intent="airRun" executionOptions={report.executionOptions}
    report={report} onClose={() => { dialogClosed += 1; }} onAuthorize={() => pendingDialog.promise}
    onAuthorized={() => { preparedCount += 1; }} onStart={async () => { startedCount += 1; return idleSenderSnapshot; }} onStarted={() => {}} />;
  if (root) render(element); else mount(element);
}

const editorDocument = { program, source: "G1 X1" };
const editorGateway = { parse: () => { const request = deferred<GcodeProgram>(); parses.push(request); return request.promise; } };
export function mountEditor(blockDelete = false) {
  const element = <ProgramEditor document={editorDocument} gateway={editorGateway} blockDelete={blockDelete} onApply={() => {}} onClose={() => {}} />;
  if (root) render(element); else mount(element);
}

export const pendingSave = deferred<void>();
export const pendingProbe = deferred<any>();
export let probeRuns = 0;
export let zeroResults = 0;
const probeGateway = { run: () => { probeRuns += 1; return pendingProbe.promise; } };
export function mountProbe(profileId = "one") {
  const element = <ZProbeDialog activeCoordinateSystem="g54" desktopRuntime gateway={probeGateway}
    heightmapGateway={previewHeightmapGateway} onClose={() => {}} onAbort={async () => snapshot}
    onError={() => {}} onSaveSettings={() => pendingSave.promise} onSnapshot={() => {}}
    onZeroEstablished={() => { zeroResults += 1; }} onUnlock={async () => snapshot} open profileId={profileId}
    probeInstalled settings={{ ...defaultZProbeSettings(), mode: "workZero", plateThicknessMm: 1 }} snapshot={snapshot} />;
  if (root) render(element); else mount(element);
}

export const sessionRead = deferred<typeof emptySurfaceSession>();
export const operationRead = deferred<typeof emptyHeightmapOperation>();
export const subscriptions: ReturnType<typeof deferred<() => void>>[] = [];
export let unsubscribeCount = 0;
export let operationListener: (value: typeof emptyHeightmapOperation) => void;
export let sessionListener: (value: typeof emptySurfaceSession) => void;
export function mountHeightmap() {
  localStorage.clear();
  mount(<HeightmapPanel activeCoordinateSystem="g54" desktopRuntime machineProfileId="one" snapshot={snapshot}
    gateway={{ ...previewHeightmapGateway, getSession: () => sessionRead.promise, getOperation: () => operationRead.promise,
      subscribeSession: (handler) => { sessionListener = handler; const d = deferred<() => void>(); subscriptions.push(d); return d.promise; },
      subscribeOperation: (handler) => { operationListener = handler; const d = deferred<() => void>(); subscriptions.push(d); return d.promise; },
    }} zProbeGateway={probeGateway} onError={() => {}} onSaveMode={async () => {}} onSnapshot={() => {}} onUnlock={async () => snapshot} />);
}
export function resolveSubscriptions() { for (const sub of subscriptions) sub.resolve(() => { unsubscribeCount += 1; }); }
export function staleHeightmapRead() {
  operationListener({ ...emptyHeightmapOperation, state: "running", progress: { measured: 1, triggered: 1, total: 9, complete: false } });
  sessionListener(emptySurfaceSession);
  operationRead.resolve(emptyHeightmapOperation); sessionRead.resolve(emptySurfaceSession);
}

export function mountPreview(revision = 0, adjustment = 0, spinning = false) {
  const element = <div style={{ width: "100%", height: "450px" }}><ToolpathPreview
    program={revision ? { ...program } : program} cuttingDepthAdjustmentMm={adjustment} onSelectSourceLine={() => {}}
    toolPosition={{ x: 10, y: 8, z: 1 }} selectedSourceLine={4} toolVisualization={{ state: spinning ? "spinning" : "selected", showCutter: true, spinning }} view="iso" /></div>;
  if (root) render(element); else mount(element);
}
