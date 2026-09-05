mod client;
mod error;
pub use client::CommandArbiter;
pub use error::ArbiterError;

mod work_coordinates;
use work_coordinates::*;
mod heightmap_operation;
use heightmap_operation::*;
mod probe_operation;
use probe_operation::*;
mod jog_operation;
use jog_operation::*;
mod homing_operation;
use homing_operation::*;
mod configuration;
use configuration::*;
mod operation_guard;
use operation_guard::*;
mod dispatch;
use dispatch::*;
mod runtime;
use runtime::*;

use std::{
    future::Future,
    time::{Duration, Instant},
};

use millo_controller::{
    Controller, ControllerConfig, ControllerError, ProgramResponsePoll, RealtimeCommand,
    UnhomedSetting,
};
use millo_domain::{
    CommandCompletion, CommandResponse, ConnectionState, ContinuousJogReceipt,
    ContinuousJogRequest, ControllerSnapshot, DeviceInspection, HardwareInspection,
    HardwareProfile, HomingRequest, HomingStartOutcome, HomingState, JogBoundarySource,
    JogPadStepOutcome, JogPadStepRequest, MachineMode, MachineOutputOutcome, MachineOutputRequest,
    OperatorConfirmation, OperatorConsoleExchange, OverrideAdjustment, Position, ProbeWorkflowMode,
    RapidOverrideTarget, ResetChallenge, ReturnToWorkOriginOutcome, ReturnToWorkOriginRequest,
    ReturnToWorkZeroOutcome, ReturnToWorkZeroRequest, SpindleControl, StepJogReceipt,
    StepJogRequest, TestJogPreparation, WorkAxis, WorkCoordinateSelectionOutcome,
    WorkCoordinateSystem, WorkZeroOutcome, WorkZeroRequest, ZProbeOutcome, ZProbeRequest,
    ZProbeSettings,
};
use millo_dry_run::{
    DryRunLineKind, DryRunPlan, DryRunPolicyError, ProgramExecutionOptions, ProgramRunPolicy,
    build_program_run_plan_with_heightmap,
};
use millo_gcode::GcodeProgram;
use millo_grbl::{
    MAX_STEP_JOG_DISTANCE_MM, MAX_STEP_JOG_FEED_MM_PER_MIN, MIN_STEP_JOG_DISTANCE_MM,
    MIN_STEP_JOG_FEED_MM_PER_MIN, active_work_coordinate_system, build_device_inspection,
    work_coordinate_parameter,
};
#[cfg(test)]
use millo_heightmap::HeightmapContactMode;
use millo_heightmap::{
    Heightmap, HeightmapCoordinateBinding, HeightmapError, HeightmapOperationSnapshot,
    HeightmapOperationState, HeightmapResumeRequest, HeightmapStartRequest, HeightmapTravelLimits,
    plan_heightmap,
};
use millo_readiness::assess;
use millo_run::program_fingerprint;
use millo_run::{
    FirstCutAuthorizationError, FirstCutConfirmation, FirstCutGate, FirstCutPreparation,
    ProgramCheckBinding, ProgramCheckCertificateError, ProgramCheckGate, ProgramRunIntent,
    RunPreflightCheck, RunPreflightLevel, RunPreflightReport, ToolChangeConfirmation,
    assess_real_run_preflight_with_options,
};
use millo_safety::{SafetyError, SafetyManager};
use millo_sender::{
    Sender, SenderError, SenderFailure, SenderFailureKind, SenderSnapshot, SenderState,
    usable_rx_buffer_capacity,
};
use millo_settings::{
    ControllerSettingEditRequest, SettingsError, VerifiedSettingUpdate, setting_values_equal,
    validate_setting_edit,
};
use millo_transport::{BoxedTransport, TransportError};
use thiserror::Error;
use tokio::{
    sync::{mpsc, oneshot, watch},
    time::{MissedTickBehavior, interval},
};

mod operator_console;
mod program_execution;

use program_execution::*;

const REQUEST_CAPACITY: usize = 32;
const WORK_ZERO_TOLERANCE_MM: f64 = 0.002;
const SENDER_RESPONSE_SLICE: Duration = Duration::from_millis(10);
const PROBE_START_SETTLE_TIMEOUT: Duration = Duration::from_secs(3);
const MOTION_SETTLE_MARGIN: Duration = Duration::from_secs(3);
const MACHINE_OPERATION_STEP_INTERVAL: Duration = Duration::from_millis(100);
const HEIGHTMAP_POSITION_TOLERANCE_MM: f64 = 0.05;
const HOMING_MIN_TIMEOUT: Duration = Duration::from_secs(30);
const HOMING_MAX_TIMEOUT: Duration = Duration::from_secs(10 * 60);
const HOMING_SETTLE_TIMEOUT: Duration = Duration::from_secs(5);
const CONTINUOUS_JOG_WATCHDOG_MARGIN: Duration = Duration::from_secs(5);
const MACHINE_BOUNDARY_MARGIN_MM: f64 = 0.05;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OperatorConsolePolicy {
    SafeOnly,
    Expert,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ExecutionTarget {
    #[default]
    Disabled,
    Mock,
    Serial,
}

impl ExecutionTarget {
    fn supports_machine_execution(self) -> bool {
        matches!(self, Self::Mock | Self::Serial)
    }
}

#[derive(Debug, Clone)]
pub struct UnhomedConfiguration {
    pub before: DeviceInspection,
    pub after: DeviceInspection,
    pub writes: Vec<CommandResponse>,
}

enum Request {
    ReplaceTransport {
        transport: BoxedTransport,
        execution_target: ExecutionTarget,
        response: oneshot::Sender<Result<ControllerSnapshot, ArbiterError>>,
    },
    Connect {
        response: oneshot::Sender<Result<ControllerSnapshot, ArbiterError>>,
    },
    SetHardwareProfile {
        profile: HardwareProfile,
        response: oneshot::Sender<Result<HardwareProfile, ArbiterError>>,
    },
    BindHardwareProfile {
        profile: HardwareProfile,
        response: oneshot::Sender<Result<HardwareProfile, ArbiterError>>,
    },
    UpdateControllerSetting {
        request: ControllerSettingEditRequest,
        response: oneshot::Sender<Result<VerifiedSettingUpdate, ArbiterError>>,
    },
    Disconnect {
        response: oneshot::Sender<Result<ControllerSnapshot, ArbiterError>>,
    },
    RefreshStatus {
        response: oneshot::Sender<Result<ControllerSnapshot, ArbiterError>>,
    },
    AcknowledgeReset {
        response: oneshot::Sender<Result<ControllerSnapshot, ArbiterError>>,
    },
    UnlockAlarm {
        operator_confirmed: bool,
        response: oneshot::Sender<Result<ControllerSnapshot, ArbiterError>>,
    },
    InspectDevice {
        response: oneshot::Sender<Result<HardwareInspection, ArbiterError>>,
    },
    ExecuteOperatorConsole {
        command: String,
        policy: OperatorConsolePolicy,
        response: oneshot::Sender<Result<OperatorConsoleExchange, ArbiterError>>,
    },
    PreflightRealRun {
        program: GcodeProgram,
        intent: ProgramRunIntent,
        execution_options: ProgramExecutionOptions,
        heightmap: Option<Heightmap>,
        response: oneshot::Sender<Result<RunPreflightReport, ArbiterError>>,
    },
    AuthorizeFirstCut {
        program: GcodeProgram,
        confirmation: FirstCutConfirmation,
        heightmap: Option<Heightmap>,
        require_check_certificate: bool,
        response: oneshot::Sender<Result<FirstCutPreparation, ArbiterError>>,
    },
    StartProgramRun {
        program: GcodeProgram,
        authorization_id: u64,
        heightmap: Option<Heightmap>,
        dispatch_immediately: bool,
        response: oneshot::Sender<Result<SenderSnapshot, ArbiterError>>,
    },
    StartCheckRun {
        program: GcodeProgram,
        execution_options: ProgramExecutionOptions,
        heightmap: Option<Heightmap>,
        response: oneshot::Sender<Result<SenderSnapshot, ArbiterError>>,
    },
    ResumeProgramRun {
        response: oneshot::Sender<Result<SenderSnapshot, ArbiterError>>,
    },
    PauseProgramRun {
        response: oneshot::Sender<Result<SenderSnapshot, ArbiterError>>,
    },
    AbortProgramRun {
        response: oneshot::Sender<Result<SenderSnapshot, ArbiterError>>,
    },
    CompleteToolChange {
        confirmation: ToolChangeConfirmation,
        response: oneshot::Sender<Result<SenderSnapshot, ArbiterError>>,
    },
    Realtime {
        command: RealtimeCommand,
        response: oneshot::Sender<Result<ControllerSnapshot, ArbiterError>>,
    },
    BeginSoftReset {
        response: oneshot::Sender<Result<ResetChallenge, ArbiterError>>,
    },
    ConfirmSoftReset {
        challenge_id: u64,
        response: oneshot::Sender<Result<ControllerSnapshot, ArbiterError>>,
    },
    PrepareTestJog {
        confirmation: OperatorConfirmation,
        response: oneshot::Sender<Result<TestJogPreparation, ArbiterError>>,
    },
    StepJog {
        request: StepJogRequest,
        response: oneshot::Sender<Result<StepJogReceipt, ArbiterError>>,
    },
    JogPadStep {
        request: JogPadStepRequest,
        response: oneshot::Sender<Result<JogPadStepOutcome, ArbiterError>>,
    },
    StartHoming {
        request: HomingRequest,
        response: oneshot::Sender<Result<HomingStartOutcome, ArbiterError>>,
    },
    StartContinuousJog {
        request: ContinuousJogRequest,
        response: oneshot::Sender<Result<ContinuousJogReceipt, ArbiterError>>,
    },
    CancelJog {
        response: oneshot::Sender<Result<ControllerSnapshot, ArbiterError>>,
    },
    SelectWorkCoordinateSystem {
        coordinate_system: WorkCoordinateSystem,
        response: oneshot::Sender<Result<WorkCoordinateSelectionOutcome, ArbiterError>>,
    },
    SetMachineOutput {
        request: MachineOutputRequest,
        response: oneshot::Sender<Result<MachineOutputOutcome, ArbiterError>>,
    },
    ConfigureUnhomedOperation {
        response: oneshot::Sender<Result<UnhomedConfiguration, ArbiterError>>,
    },
    SetWorkZero {
        request: WorkZeroRequest,
        response: oneshot::Sender<Result<WorkZeroOutcome, ArbiterError>>,
    },
    ReturnToWorkZero {
        request: ReturnToWorkZeroRequest,
        response: oneshot::Sender<Result<ReturnToWorkZeroOutcome, ArbiterError>>,
    },
    ReturnToWorkOrigin {
        request: ReturnToWorkOriginRequest,
        response: oneshot::Sender<Result<ReturnToWorkOriginOutcome, ArbiterError>>,
    },
    ProbeZ {
        request: ZProbeRequest,
        response: oneshot::Sender<Result<ZProbeOutcome, ArbiterError>>,
    },
    PrepareHeightmap {
        request: HeightmapStartRequest,
        response: oneshot::Sender<Result<HeightmapOperationSnapshot, ArbiterError>>,
    },
    PrepareResumeHeightmap {
        map: Heightmap,
        request: HeightmapResumeRequest,
        response: oneshot::Sender<Result<HeightmapOperationSnapshot, ArbiterError>>,
    },
    CommitPreparedHeightmap {
        operation_sequence: u64,
        response: oneshot::Sender<Result<HeightmapOperationSnapshot, ArbiterError>>,
    },
    DiscardPreparedHeightmap {
        operation_sequence: u64,
        response: oneshot::Sender<Result<(), ArbiterError>>,
    },
    PauseHeightmap {
        response: oneshot::Sender<Result<HeightmapOperationSnapshot, ArbiterError>>,
    },
    ResumeHeightmap {
        response: oneshot::Sender<Result<HeightmapOperationSnapshot, ArbiterError>>,
    },
    CancelHeightmap {
        response: oneshot::Sender<Result<HeightmapOperationSnapshot, ArbiterError>>,
    },
    StartDryRun {
        plan: DryRunPlan,
        response: oneshot::Sender<Result<SenderSnapshot, ArbiterError>>,
    },
    PauseDryRun {
        response: oneshot::Sender<Result<SenderSnapshot, ArbiterError>>,
    },
    ResumeDryRun {
        response: oneshot::Sender<Result<SenderSnapshot, ArbiterError>>,
    },
    CancelDryRun {
        response: oneshot::Sender<Result<SenderSnapshot, ArbiterError>>,
    },
    CommitPreparedProgramRun {
        run_sequence: u64,
        response: oneshot::Sender<Result<SenderSnapshot, ArbiterError>>,
    },
    DiscardPreparedProgramRun {
        run_sequence: u64,
        response: oneshot::Sender<Result<SenderSnapshot, ArbiterError>>,
    },
}

struct ActorState {
    controller: Controller<BoxedTransport>,
    config: ControllerConfig,
    hardware_profile: HardwareProfile,
    execution_target: ExecutionTarget,
    sender: Sender,
    sender_dispatch_enabled: bool,
    safety: SafetyManager,
    first_cut: FirstCutGate,
    program_check: ProgramCheckGate,
    pending_program_check: Option<ProgramCheckBinding>,
    verified_z_datum: Option<VerifiedZDatum>,
    active_homing: Option<ActiveHoming>,
    homing_sequence: u64,
    machine_envelope: Option<MachineEnvelope>,
    active_continuous_jog: Option<ActiveContinuousJog>,
    active_z_probe: Option<ActiveZProbe>,
    prepared_heightmap: Option<ActiveHeightmap>,
    active_heightmap: Option<ActiveHeightmap>,
    heightmap_sequence: u64,
    snapshots: watch::Sender<ControllerSnapshot>,
    sender_snapshots: watch::Sender<SenderSnapshot>,
    heightmap_snapshots: watch::Sender<HeightmapOperationSnapshot>,
}

#[derive(Clone, Copy)]
struct VerifiedZDatum {
    binding: HeightmapCoordinateBinding,
    reset_count: u64,
    reconnect_count: u32,
}

struct ActiveHoming {
    started: Instant,
    timeout: Duration,
    settling_since: Option<Instant>,
    travel_mm: [f64; 3],
    direction_mask: u8,
    pull_off_mm: f64,
}

#[derive(Clone, Copy)]
struct MachineEnvelope {
    ranges: [(f64, f64); 3],
}

struct ActiveContinuousJog {
    deadline: Instant,
    cancel_requested: bool,
}

struct ActiveZProbe {
    request: ZProbeRequest,
    coordinate_system: WorkCoordinateSystem,
    restore_modal: String,
    command: String,
    response: oneshot::Sender<Result<ZProbeOutcome, ArbiterError>>,
}

struct StartedZProbe {
    request: ZProbeRequest,
    coordinate_system: WorkCoordinateSystem,
    restore_modal: String,
    command: String,
}

struct ActiveHeightmap {
    map: Heightmap,
    coordinate_system: WorkCoordinateSystem,
    restore_modal: String,
    next_sequence: usize,
    phase: HeightmapPhase,
    paused: bool,
    operation_sequence: u64,
    start_work_xy: Option<(f64, f64)>,
    last_work_xy: Option<(f64, f64)>,
    last_work_z: Option<f64>,
    highest_measured_surface_z: f64,
    establish_z_zero_on_first_contact: bool,
}

enum HeightmapPhase {
    Raise,
    WaitForRaise {
        started: Instant,
        timeout: Duration,
        target_z: f64,
    },
    MoveXy,
    WaitForXy {
        started: Instant,
        timeout: Duration,
        target_x: f64,
        target_y: f64,
    },
    Probe,
    PollProbe {
        command: String,
    },
    WaitForProbeIdle {
        started: Instant,
    },
    RecordProbe,
    ReturnToStartXy,
    WaitForReturnXy {
        started: Instant,
        timeout: Duration,
        target_x: f64,
        target_y: f64,
    },
    Finalize,
}

const fn work_axis_index(axis: WorkAxis) -> usize {
    match axis {
        WorkAxis::X => 0,
        WorkAxis::Y => 1,
        WorkAxis::Z => 2,
    }
}

#[cfg(test)]
mod tests;
