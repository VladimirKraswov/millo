use super::*;

#[derive(Debug, Error)]
pub enum ArbiterError {
    #[error("Четырёхосевая программа не готова: {0}")]
    RotaryProgramUnavailable(String),
    #[error(transparent)]
    Controller(#[from] ControllerError),
    #[error(transparent)]
    Safety(#[from] SafetyError),
    #[error("command arbiter is no longer running")]
    Closed,
    #[error("command arbiter dropped a response")]
    ResponseDropped,
    #[error("jog cancel requires Jog state, current mode is {0:?}")]
    JogCancelUnavailable(MachineMode),
    #[error("unhomed configuration verification failed: {0}")]
    ConfigurationVerification(String),
    #[error("jog distance must be between 0.01 and 100000 mm")]
    JogPadDistanceOutOfRange,
    #[error("jog feed must be between 10 and 100000 mm/min")]
    JogPadFeedOutOfRange,
    #[error("jog feed {requested:.0} mm/min exceeds {axis:?} maximum rate {maximum:.0} mm/min")]
    JogPadFeedExceedsAxisRate {
        axis: millo_domain::JogAxis,
        requested: f64,
        maximum: f64,
    },
    #[error("jog distance {requested:.3} mm exceeds the {axis:?} profile limit {maximum:.3} mm")]
    JogPadDistanceExceedsProfile {
        axis: millo_domain::JogAxis,
        requested: f64,
        maximum: f64,
    },
    #[error("continuous jog direction must be -1 or 1")]
    ContinuousJogDirectionInvalid,
    #[error("continuous jog is already active")]
    ContinuousJogActive,
    #[error("axis {0:?} is not enabled by the selected machine profile")]
    JogAxisUnavailable(millo_domain::JogAxis),
    #[error("no safe continuous-jog distance remains before the {axis:?} boundary")]
    ContinuousJogBoundaryReached { axis: millo_domain::JogAxis },
    #[error("homing requires explicit operator confirmation")]
    HomingConfirmationRequired,
    #[error("homing is not enabled in the selected machine profile")]
    HomingNotInstalled,
    #[error("GRBL homing is disabled ($22 must be 1)")]
    HomingDisabled,
    #[error("homing can start only from Idle or Alarm, current mode is {0:?}")]
    HomingUnavailable(MachineMode),
    #[error("homing did not settle to Idle within {0} ms")]
    HomingSettleTimeout(u64),
    #[error("work coordinate system verification failed: expected {expected:?}, read {actual:?}")]
    WorkCoordinateSelectionVerification {
        expected: WorkCoordinateSystem,
        actual: Option<WorkCoordinateSystem>,
    },
    #[error("controller-managed spindle output is disabled in the selected machine profile")]
    ControllerSpindleDisabled,
    #[error("{0} coolant output is disabled in the selected machine profile")]
    CoolantOutputDisabled(&'static str),
    #[error(
        "spindle speed must be finite and within controller range {minimum:.0}..{maximum:.0} rpm"
    )]
    SpindleSpeedOutOfRange { minimum: f64, maximum: f64 },
    #[error("machine output verification failed: {0}")]
    MachineOutputVerification(String),
    #[error("work zero requires explicit operator position confirmation")]
    WorkZeroConfirmationRequired,
    #[error("active work coordinate system is not one of G54-G59")]
    ActiveWorkCoordinateSystemUnavailable,
    #[error("alarm unlock requires explicit operator confirmation")]
    UnlockConfirmationRequired,
    #[error("work zero verification failed: {0}")]
    WorkZeroVerification(String),
    #[error("Z probe requires confirmation that the plate is secured and the spindle is stopped")]
    ZProbeConfirmationRequired,
    #[error("the selected machine profile does not declare an installed probe")]
    ZProbeNotInstalled,
    #[error("the selected Z probe mode is disabled in the machine profile")]
    ZProbeDisabled,
    #[error("probe input P is already active; open the circuit before probing")]
    ZProbeInputAlreadyActive,
    #[error("invalid Z probe settings: {0}")]
    InvalidZProbeSettings(&'static str),
    #[error("Z probe result could not be verified: {0}")]
    ZProbeVerification(String),
    #[error("probe did not contact the surface within the configured search range")]
    ZProbeContactNotFound,
    #[error(
        "Z probe motion did not settle to Idle within {timeout_ms} ms (last mode {last_mode:?})"
    )]
    ZProbeSettleTimeout {
        timeout_ms: u64,
        last_mode: MachineMode,
    },
    #[error("Z probe retract did not return to Idle within {0} ms")]
    ZProbeRetractTimeout(u64),
    #[error("Z probe was interrupted by controller reset")]
    ZProbeReset,
    #[error(
        "probe start is blocked: connection {connection:?}, mode {mode:?}, alarm {alarm_active}, reset acknowledgement pending {reset_pending}"
    )]
    ProbeStartBlocked {
        connection: ConnectionState,
        mode: MachineMode,
        alarm_active: bool,
        reset_pending: bool,
    },
    #[error(
        "probe start timed out after {timeout_ms} ms waiting for Idle (last mode {last_mode:?})"
    )]
    ProbeStartSettleTimeout {
        timeout_ms: u64,
        last_mode: MachineMode,
    },
    #[error(transparent)]
    Heightmap(#[from] HeightmapError),
    #[error("heightmap requires confirmation that the entire perimeter is clear and reachable")]
    HeightmapConfirmationRequired,
    #[error("heightmap mode is not selected in the machine profile")]
    HeightmapModeDisabled,
    #[error("a fixed contact plate must cover every probing point")]
    HeightmapContactUnavailable,
    #[error("another machine operation is active")]
    MachineOperationBusy,
    #[error("operator console command was rejected by the active command policy")]
    OperatorConsoleCommandRejected,
    #[error("controller query is unavailable in {0:?}; wait for Idle or use ? for status")]
    OperatorConsoleQueryUnavailable(MachineMode),
    #[error("heightmap operation is not running")]
    HeightmapOperationUnavailable,
    #[error("prepared heightmap operation is unavailable")]
    PreparedHeightmapUnavailable,
    #[error("prepared heightmap {expected} does not match active preparation {actual}")]
    PreparedHeightmapMismatch { expected: u64, actual: u64 },
    #[error("heightmap probing failed: {0}")]
    HeightmapProbe(String),
    #[error("heightmap {axis} movement ended at {actual:.3} mm, expected {expected:.3} mm")]
    HeightmapPositionVerification {
        axis: &'static str,
        expected: f64,
        actual: f64,
    },
    #[error("current work position is unavailable")]
    WorkPositionUnavailable,
    #[error("raise work Z above zero before returning {0:?} to zero")]
    ReturnToZeroNeedsClearance(WorkAxis),
    #[error("return distance {requested:.3} mm exceeds the {axis:?} travel {maximum:.3} mm")]
    ReturnToZeroDistanceExceedsProfile {
        axis: WorkAxis,
        requested: f64,
        maximum: f64,
    },
    #[error(transparent)]
    Sender(#[from] SenderError),
    #[error("dry run is disabled for the active transport")]
    DryRunTransportUnavailable,
    #[error("program execution is disabled for the active transport target")]
    RealRunTransportUnavailable,
    #[error("GRBL Check is disabled for the active transport target")]
    CheckRunTransportUnavailable,
    #[error("program run can resume only from GRBL Hold or Idle, current mode is {0:?}")]
    ProgramRunResumeUnavailable(MachineMode),
    #[error("physical program pause is unavailable while sender is {0:?}")]
    ProgramRunPauseUnavailable(SenderState),
    #[error("physical program stop is unavailable while sender is {0:?}")]
    ProgramRunStopUnavailable(SenderState),
    #[error("tool change can be completed only at an active M6 barrier, sender is {0:?}")]
    ToolChangeUnavailable(SenderState),
    #[error("tool-change confirmation does not match the active source line or requested tool")]
    ToolChangeMismatch,
    #[error("tool-change confirmation is incomplete: {0:?}")]
    ToolChangeConfirmationIncomplete(Vec<&'static str>),
    #[error("tool change can continue only from fresh GRBL Idle, current mode is {0:?}")]
    ToolChangeControllerUnavailable(MachineMode),
    #[error("a physical program run can be stopped only with Feed Hold followed by Soft Reset")]
    ProgramRunStopRequiresReset,
    #[error("prepared program run {expected} does not match active run {actual}")]
    PreparedRunMismatch { expected: u64, actual: u64 },
    #[error("prepared program run is unavailable while sender is {0:?}")]
    PreparedRunUnavailable(SenderState),
    #[error("prepared program run has already been committed to dispatch")]
    PreparedRunAlreadyCommitted,
    #[error(transparent)]
    FirstCut(#[from] FirstCutAuthorizationError),
    #[error(transparent)]
    RunPolicy(#[from] DryRunPolicyError),
    #[error("machine profile can be changed only while disconnected, current state is {0:?}")]
    ProfileChangeUnavailable(ConnectionState),
    #[error("transport can be replaced only while disconnected, current state is {0:?}")]
    TransportReplacementUnavailable(ConnectionState),
    #[error("connect requires a disconnected controller, current state is {0:?}")]
    ConnectUnavailable(ConnectionState),
    #[error(transparent)]
    Settings(#[from] SettingsError),
    #[error(
        "controller setting verification failed for {key}: requested {requested}, read {stored}"
    )]
    SettingVerification {
        key: String,
        requested: String,
        stored: String,
    },
    #[error("validated controller setting {0} disappeared from the inspection snapshot")]
    SettingSourceMissing(String),
}
