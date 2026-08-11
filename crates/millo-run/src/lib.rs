use std::time::{Duration, Instant};

use millo_domain::{
    ConnectionState, ControllerSnapshot, HardwareInspection, MachineMode, Position, ReadinessLevel,
    SpindleControl,
};
use millo_dry_run::{DryRunBlocker, DryRunBlockerKind, DryRunPolicyError, build_dry_run_plan};
use millo_gcode::{GcodeProgram, ProgramBounds, ToolpathKind};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

const MAX_REPORTED_PROGRAM_BLOCKERS: usize = 20;
const FIRST_CUT_POSITION_TOLERANCE_MM: f64 = 0.002;
pub const FIRST_CUT_AUTHORIZATION_TTL: Duration = Duration::from_secs(30);

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FirstCutConfirmation {
    pub stock_secured: bool,
    pub tool_secured: bool,
    pub xyz_zero_verified: bool,
    pub safe_z_verified: bool,
    pub manual_spindle_running: bool,
    pub power_control_reachable: bool,
}

impl FirstCutConfirmation {
    pub fn is_complete(self) -> bool {
        self.missing().is_empty()
    }

    pub fn missing(self) -> Vec<&'static str> {
        [
            (!self.stock_secured).then_some("stock secured"),
            (!self.tool_secured).then_some("tool secured"),
            (!self.xyz_zero_verified).then_some("XYZ zero verified"),
            (!self.safe_z_verified).then_some("safe Z verified"),
            (!self.manual_spindle_running).then_some("manual spindle running"),
            (!self.power_control_reachable).then_some("power control reachable"),
        ]
        .into_iter()
        .flatten()
        .collect()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FirstCutAuthorization {
    pub id: u64,
    pub expires_in_ms: u64,
    pub source_name: String,
    pub program_fingerprint: String,
    pub poll_sequence: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FirstCutPreparation {
    pub report: RunPreflightReport,
    pub authorization: FirstCutAuthorization,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum FirstCutAuthorizationError {
    #[error("first-cut confirmation is incomplete: {missing:?}")]
    IncompleteConfirmation { missing: Vec<&'static str> },
    #[error("real-run preflight contains {blockers} blocker(s)")]
    PreflightBlocked { blockers: usize },
    #[error("preflight evidence does not match the current controller snapshot")]
    StalePreflightEvidence,
    #[error("controller is not connected and idle")]
    UnsafeControllerState,
    #[error("controller did not report an XYZ position for first-cut authorization")]
    ControllerPositionUnavailable,
    #[error("first-cut authorization is missing")]
    AuthorizationMissing,
    #[error("first-cut authorization does not match the active lease")]
    AuthorizationMismatch,
    #[error("first-cut authorization expired")]
    AuthorizationExpired,
    #[error("controller session changed after first-cut authorization")]
    ControllerSessionChanged,
    #[error("controller position changed after first-cut authorization")]
    ControllerPositionChanged,
    #[error("program changed after first-cut authorization")]
    ProgramChanged,
}

#[derive(Debug)]
struct FirstCutLease {
    authorization: FirstCutAuthorization,
    expires_at: Instant,
    reset_count: u64,
    reconnect_count: u32,
    machine_position: Option<Position>,
    work_position: Option<Position>,
    work_coordinate_offset: Option<Position>,
}

#[derive(Debug, Default)]
pub struct FirstCutGate {
    next_id: u64,
    lease: Option<FirstCutLease>,
}

impl FirstCutGate {
    pub fn authorize(
        &mut self,
        confirmation: FirstCutConfirmation,
        report: &RunPreflightReport,
        snapshot: &ControllerSnapshot,
        now: Instant,
    ) -> Result<FirstCutAuthorization, FirstCutAuthorizationError> {
        self.lease = None;
        let missing = confirmation.missing();
        if !missing.is_empty() {
            return Err(FirstCutAuthorizationError::IncompleteConfirmation { missing });
        }
        if !report.ready {
            return Err(FirstCutAuthorizationError::PreflightBlocked {
                blockers: report.blocker_count,
            });
        }
        if report.poll_sequence != snapshot.poll_sequence {
            return Err(FirstCutAuthorizationError::StalePreflightEvidence);
        }
        ensure_stable_idle(snapshot)?;
        if snapshot.machine.machine_position.is_none() && snapshot.machine.work_position.is_none() {
            return Err(FirstCutAuthorizationError::ControllerPositionUnavailable);
        }

        self.next_id = self.next_id.saturating_add(1);
        let authorization = FirstCutAuthorization {
            id: self.next_id,
            expires_in_ms: duration_ms(FIRST_CUT_AUTHORIZATION_TTL),
            source_name: report.source_name.clone(),
            program_fingerprint: report.program_fingerprint.clone(),
            poll_sequence: report.poll_sequence,
        };
        self.lease = Some(FirstCutLease {
            authorization: authorization.clone(),
            expires_at: now + FIRST_CUT_AUTHORIZATION_TTL,
            reset_count: snapshot.reset_count,
            reconnect_count: snapshot.reconnect_count,
            machine_position: snapshot.machine.machine_position,
            work_position: snapshot.machine.work_position,
            work_coordinate_offset: snapshot.machine.work_coordinate_offset,
        });
        Ok(authorization)
    }

    pub fn consume(
        &mut self,
        authorization_id: u64,
        program_fingerprint: &str,
        snapshot: &ControllerSnapshot,
        now: Instant,
    ) -> Result<(), FirstCutAuthorizationError> {
        let Some(lease) = self.lease.take() else {
            return Err(FirstCutAuthorizationError::AuthorizationMissing);
        };
        if lease.authorization.id != authorization_id {
            return Err(FirstCutAuthorizationError::AuthorizationMismatch);
        }
        if now > lease.expires_at {
            return Err(FirstCutAuthorizationError::AuthorizationExpired);
        }
        ensure_stable_idle(snapshot)?;
        if lease.reset_count != snapshot.reset_count
            || lease.reconnect_count != snapshot.reconnect_count
        {
            return Err(FirstCutAuthorizationError::ControllerSessionChanged);
        }
        if !positions_equal(lease.machine_position, snapshot.machine.machine_position)
            || !positions_equal(lease.work_position, snapshot.machine.work_position)
            || !positions_equal(
                lease.work_coordinate_offset,
                snapshot.machine.work_coordinate_offset,
            )
        {
            return Err(FirstCutAuthorizationError::ControllerPositionChanged);
        }
        if lease.authorization.program_fingerprint != program_fingerprint {
            return Err(FirstCutAuthorizationError::ProgramChanged);
        }
        Ok(())
    }

    pub fn observe(&mut self, snapshot: &ControllerSnapshot, now: Instant) {
        let Some(lease) = self.lease.as_ref() else {
            return;
        };
        let valid = now <= lease.expires_at
            && stable_idle(snapshot)
            && lease.reset_count == snapshot.reset_count
            && lease.reconnect_count == snapshot.reconnect_count
            && positions_equal(lease.machine_position, snapshot.machine.machine_position)
            && positions_equal(lease.work_position, snapshot.machine.work_position)
            && positions_equal(
                lease.work_coordinate_offset,
                snapshot.machine.work_coordinate_offset,
            );
        if !valid {
            self.lease = None;
        }
    }

    pub fn invalidate(&mut self) {
        self.lease = None;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum RunPreflightLevel {
    Pass,
    Caution,
    Blocker,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RunPreflightCheck {
    pub id: String,
    pub level: RunPreflightLevel,
    pub title: String,
    pub detail: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_line: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RunProgramBlocker {
    pub kind: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_line: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RunPreflightReport {
    pub source_name: String,
    pub program_fingerprint: String,
    pub ready: bool,
    pub blocker_count: usize,
    pub caution_count: usize,
    pub poll_sequence: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bounds: Option<ProgramBounds>,
    pub hardware: HardwareInspection,
    pub checks: Vec<RunPreflightCheck>,
    pub program_blockers: Vec<RunProgramBlocker>,
    pub total_program_blockers: usize,
}

pub fn assess_real_run_preflight(
    program: &GcodeProgram,
    hardware: HardwareInspection,
    snapshot: &ControllerSnapshot,
) -> RunPreflightReport {
    let mut checks = Vec::new();
    let stable_idle = snapshot.connection == ConnectionState::Connected
        && snapshot.machine.mode == MachineMode::Idle
        && snapshot.alarm.is_none()
        && snapshot.reset_notice.is_none();
    checks.push(check(
        "controller-state",
        if stable_idle {
            RunPreflightLevel::Pass
        } else {
            RunPreflightLevel::Blocker
        },
        "Fresh controller state",
        if stable_idle {
            format!("Connected · Idle · status #{}", snapshot.poll_sequence)
        } else {
            format!(
                "A fresh Connected + Idle state is required; current mode is {}",
                snapshot.machine.reported_mode
            )
        },
        None,
    ));

    let motion_hardware_blockers = hardware
        .readiness
        .checks
        .iter()
        .filter(|item| {
            item.level == ReadinessLevel::Blocker
                && !matches!(item.id.as_str(), "controller-state" | "probe-input")
        })
        .count();
    checks.push(check(
        "motion-hardware",
        if motion_hardware_blockers == 0 {
            RunPreflightLevel::Pass
        } else {
            RunPreflightLevel::Blocker
        },
        "Motion configuration",
        if motion_hardware_blockers == 0 {
            "Firmware, XYZ tuning, limits profile, units and milling mode passed".to_owned()
        } else {
            format!("{motion_hardware_blockers} motion-critical Device Inspector check(s) failed")
        },
        None,
    ));

    let (policy_blockers, empty_program) = match build_dry_run_plan(program) {
        Ok(_) => (Vec::new(), false),
        Err(DryRunPolicyError::Rejected(_, blockers)) => (blockers, false),
        Err(DryRunPolicyError::EmptyProgram) => (Vec::new(), true),
    };
    let first_policy_blocker = policy_blockers.first();
    checks.push(check(
        "program-policy",
        if policy_blockers.is_empty() && !empty_program {
            RunPreflightLevel::Pass
        } else {
            RunPreflightLevel::Blocker
        },
        "Motion-only program policy",
        if empty_program {
            "The program has no executable lines".to_owned()
        } else if let Some(blocker) = first_policy_blocker {
            format!(
                "{} blocked command(s); first: {}",
                policy_blockers.len(),
                blocker.message
            )
        } else {
            "No spindle activation, coolant, probing, tool change, coordinate mutation or machine-coordinate motion"
                .to_owned()
        },
        first_policy_blocker.and_then(|blocker| blocker.source_line),
    ));

    let modal_contract = assess_modal_contract(program);
    checks.push(check(
        "program-modal-contract",
        if modal_contract.missing.is_empty() {
            RunPreflightLevel::Pass
        } else {
            RunPreflightLevel::Blocker
        },
        "Explicit program modes",
        if modal_contract.missing.is_empty() {
            "Required units, distance, feed and plane modes are declared before motion".to_owned()
        } else {
            format!(
                "Declare {} before the corresponding first motion",
                modal_contract.missing.join(", ")
            )
        },
        modal_contract.source_line,
    ));

    let geometry_ready = program.summary.preview_complete
        && program.summary.motion_count > 0
        && program.summary.bounds.is_some();
    checks.push(check(
        "program-geometry",
        if geometry_ready {
            RunPreflightLevel::Pass
        } else {
            RunPreflightLevel::Blocker
        },
        "Program geometry",
        if geometry_ready {
            let bounds = program.summary.bounds.expect("checked bounds");
            format!(
                "{} motion(s) · {:.3} × {:.3} × {:.3} mm",
                program.summary.motion_count, bounds.size.x, bounds.size.y, bounds.size.z
            )
        } else {
            "A complete preview with at least one bounded motion is required".to_owned()
        },
        None,
    ));

    let active_wcs = hardware.device.modal_state.iter().find(|value| {
        matches!(
            value.as_str(),
            "G54" | "G55" | "G56" | "G57" | "G58" | "G59"
        )
    });
    checks.push(check(
        "work-coordinate-system",
        if active_wcs.is_some() {
            RunPreflightLevel::Pass
        } else {
            RunPreflightLevel::Blocker
        },
        "Work coordinate system",
        active_wcs.map_or_else(
            || "Device Inspector did not report an active G54-G59 system".to_owned(),
            |value| format!("{value} is active; work zero must be verified before authorization"),
        ),
        None,
    ));

    if !hardware.readiness.profile.homing_installed
        || !hardware.readiness.profile.limit_switches_installed
    {
        checks.push(check(
            "unhomed-envelope",
            RunPreflightLevel::Caution,
            "Unverified machine envelope",
            "Without homing and limit switches, preview bounds do not prove physical clearance"
                .to_owned(),
            None,
        ));
    }
    if hardware.readiness.profile.spindle_control == SpindleControl::Manual {
        checks.push(check(
            "manual-spindle",
            RunPreflightLevel::Caution,
            "Manual spindle workflow",
            "Automatic M3/M4/S commands remain forbidden; spindle state requires a separate operator step"
                .to_owned(),
            None,
        ));
    }
    checks.push(check(
        "operator-setup",
        RunPreflightLevel::Caution,
        "Physical setup not authorized",
        "Stock, cutter, work zero, safe Z and dry-run clearance still require operator confirmation"
            .to_owned(),
        None,
    ));

    let blocker_count = checks
        .iter()
        .filter(|item| item.level == RunPreflightLevel::Blocker)
        .count();
    let caution_count = checks
        .iter()
        .filter(|item| item.level == RunPreflightLevel::Caution)
        .count();
    let total_program_blockers = policy_blockers.len();
    let program_blockers = policy_blockers
        .into_iter()
        .take(MAX_REPORTED_PROGRAM_BLOCKERS)
        .map(program_blocker)
        .collect();

    RunPreflightReport {
        source_name: program.source_name.clone(),
        program_fingerprint: program_fingerprint(program),
        ready: blocker_count == 0,
        blocker_count,
        caution_count,
        poll_sequence: snapshot.poll_sequence,
        bounds: program.summary.bounds,
        hardware,
        checks,
        program_blockers,
        total_program_blockers,
    }
}

pub fn program_fingerprint(program: &GcodeProgram) -> String {
    let mut digest = Sha256::new();
    update_digest_field(&mut digest, program.source_name.as_bytes());
    for line in &program.lines {
        update_digest_field(&mut digest, line.source.as_bytes());
    }
    format!("{:x}", digest.finalize())
}

fn update_digest_field(digest: &mut Sha256, value: &[u8]) {
    digest.update((value.len() as u64).to_be_bytes());
    digest.update(value);
}

fn ensure_stable_idle(snapshot: &ControllerSnapshot) -> Result<(), FirstCutAuthorizationError> {
    if stable_idle(snapshot) {
        Ok(())
    } else {
        Err(FirstCutAuthorizationError::UnsafeControllerState)
    }
}

fn stable_idle(snapshot: &ControllerSnapshot) -> bool {
    snapshot.connection == ConnectionState::Connected
        && snapshot.machine.mode == MachineMode::Idle
        && snapshot.alarm.is_none()
        && snapshot.reset_notice.is_none()
}

fn positions_equal(left: Option<Position>, right: Option<Position>) -> bool {
    match (left, right) {
        (Some(left), Some(right)) => {
            coordinate_equal(left.x, right.x)
                && coordinate_equal(left.y, right.y)
                && coordinate_equal(left.z, right.z)
                && match (left.a, right.a) {
                    (Some(left), Some(right)) => coordinate_equal(left, right),
                    (None, None) => true,
                    _ => false,
                }
        }
        (None, None) => true,
        _ => false,
    }
}

fn coordinate_equal(left: f64, right: f64) -> bool {
    left.is_finite() && right.is_finite() && (left - right).abs() <= FIRST_CUT_POSITION_TOLERANCE_MM
}

fn duration_ms(duration: Duration) -> u64 {
    duration.as_millis().try_into().unwrap_or(u64::MAX)
}

fn program_blocker(blocker: DryRunBlocker) -> RunProgramBlocker {
    RunProgramBlocker {
        kind: blocker_kind_id(blocker.kind).to_owned(),
        message: blocker.message,
        source_line: blocker.source_line,
    }
}

fn blocker_kind_id(kind: DryRunBlockerKind) -> &'static str {
    match kind {
        DryRunBlockerKind::SpindleActivation => "spindle-activation",
        DryRunBlockerKind::SpindleSpeed => "spindle-speed",
        DryRunBlockerKind::CoolantActivation => "coolant-activation",
        DryRunBlockerKind::ProbeCycle => "probe-cycle",
        DryRunBlockerKind::ToolChange => "tool-change",
        DryRunBlockerKind::MachineCoordinateMotion => "machine-coordinate-motion",
        DryRunBlockerKind::CoordinateMutation => "coordinate-mutation",
        DryRunBlockerKind::UnsupportedProgram => "unsupported-program",
        DryRunBlockerKind::IncompletePreview => "incomplete-preview",
        DryRunBlockerKind::CommandTooLong => "command-too-long",
    }
}

struct ModalContract {
    missing: Vec<&'static str>,
    source_line: Option<usize>,
}

fn assess_modal_contract(program: &GcodeProgram) -> ModalContract {
    let first_motion = program
        .toolpath
        .iter()
        .map(|segment| segment.source_line)
        .min();
    let first_feed_motion = program
        .toolpath
        .iter()
        .filter(|segment| segment.kind != ToolpathKind::Rapid)
        .map(|segment| segment.source_line)
        .min();
    let first_arc = program
        .toolpath
        .iter()
        .filter(|segment| {
            matches!(
                segment.kind,
                ToolpathKind::ArcClockwise | ToolpathKind::ArcCounterclockwise
            )
        })
        .map(|segment| segment.source_line)
        .min();
    let mut missing = Vec::new();

    if let Some(source_line) = first_motion {
        if last_modal_code(program, source_line, &[20, 21]) != Some(21) {
            missing.push("G21");
        }
        if last_modal_code(program, source_line, &[90, 91]) != Some(90) {
            missing.push("G90");
        }
    }
    if let Some(source_line) = first_feed_motion
        && last_modal_code(program, source_line, &[93, 94]) != Some(94)
    {
        missing.push("G94");
    }
    if let Some(source_line) = first_arc
        && last_modal_code(program, source_line, &[17, 18, 19]) != Some(17)
    {
        missing.push("G17");
    }

    ModalContract {
        missing,
        source_line: first_motion,
    }
}

fn last_modal_code(program: &GcodeProgram, through_line: usize, family: &[u16]) -> Option<u16> {
    let mut active = None;
    for line in program
        .lines
        .iter()
        .take_while(|line| line.source_line <= through_line)
    {
        for word in line.normalized.split_whitespace() {
            let Some(value) = word
                .strip_prefix('G')
                .and_then(|value| value.parse::<f64>().ok())
            else {
                continue;
            };
            if value.fract().abs() > f64::EPSILON {
                continue;
            }
            let code = value as u16;
            if family.contains(&code) {
                active = Some(code);
            }
        }
    }
    active
}

fn check(
    id: &str,
    level: RunPreflightLevel,
    title: &str,
    detail: String,
    source_line: Option<usize>,
) -> RunPreflightCheck {
    RunPreflightCheck {
        id: id.to_owned(),
        level,
        title: title.to_owned(),
        detail,
        source_line,
    }
}

#[cfg(test)]
mod tests {
    use millo_domain::{
        ControllerSnapshot, DeviceInspection, HardwareProfile, MachineState, ReadinessCheck,
        ReadinessReport,
    };
    use millo_gcode::{ProgramParseRequest, parse_program};

    use super::*;

    fn program(source: &str) -> GcodeProgram {
        parse_program(ProgramParseRequest {
            source_name: "first-cut.nc".to_owned(),
            source: source.to_owned(),
        })
        .unwrap()
    }

    fn snapshot(mode: MachineMode) -> ControllerSnapshot {
        ControllerSnapshot {
            connection: ConnectionState::Connected,
            machine: MachineState {
                mode,
                reported_mode: if mode == MachineMode::Idle {
                    "Idle".to_owned()
                } else {
                    "Run".to_owned()
                },
                machine_position: Some(Position::default()),
                work_position: Some(Position::default()),
                ..MachineState::default()
            },
            poll_sequence: 42,
            ..ControllerSnapshot::default()
        }
    }

    fn hardware(checks: Vec<ReadinessCheck>) -> HardwareInspection {
        let blocker_count = checks
            .iter()
            .filter(|item| item.level == ReadinessLevel::Blocker)
            .count();
        HardwareInspection {
            device: DeviceInspection {
                modal_state: vec![
                    "G0".to_owned(),
                    "G54".to_owned(),
                    "G21".to_owned(),
                    "G90".to_owned(),
                    "M5".to_owned(),
                ],
                ..DeviceInspection::default()
            },
            readiness: ReadinessReport {
                profile: HardwareProfile::first_machine(),
                test_jog_ready: blocker_count == 0,
                probe_ready: false,
                blocker_count,
                caution_count: 0,
                checks,
            },
        }
    }

    fn readiness_check(id: &str, level: ReadinessLevel) -> ReadinessCheck {
        ReadinessCheck {
            id: id.to_owned(),
            level,
            title: id.to_owned(),
            detail: id.to_owned(),
            evidence: None,
        }
    }

    fn first_cut_confirmation() -> FirstCutConfirmation {
        FirstCutConfirmation {
            stock_secured: true,
            tool_secured: true,
            xyz_zero_verified: true,
            safe_z_verified: true,
            manual_spindle_running: true,
            power_control_reachable: true,
        }
    }

    fn ready_first_cut(snapshot: &ControllerSnapshot) -> (GcodeProgram, RunPreflightReport) {
        let program = program("G21 G90 G94\nG0 Z2\nG1 X5 Y2 F50\nM5");
        let report = assess_real_run_preflight(
            &program,
            hardware(vec![readiness_check("axis-steps", ReadinessLevel::Pass)]),
            snapshot,
        );
        assert!(report.ready);
        (program, report)
    }

    #[test]
    fn clears_a_motion_only_program_but_retains_operator_cautions() {
        let report = assess_real_run_preflight(
            &program("G21 G90 G94\nG0 Z2\nG1 X5 Y2 F50\nM5"),
            hardware(vec![readiness_check("axis-steps", ReadinessLevel::Pass)]),
            &snapshot(MachineMode::Idle),
        );

        assert!(report.ready);
        assert_eq!(report.blocker_count, 0);
        assert_eq!(report.caution_count, 3);
        assert_eq!(report.poll_sequence, 42);
        assert_eq!(report.total_program_blockers, 0);
    }

    #[test]
    fn retains_the_exact_source_line_for_forbidden_spindle_control() {
        let report = assess_real_run_preflight(
            &program("G21 G90 G94\nM3 S12000\nG1 X1 F50"),
            hardware(Vec::new()),
            &snapshot(MachineMode::Idle),
        );

        assert!(!report.ready);
        assert_eq!(report.program_blockers[0].source_line, Some(2));
        assert!(
            report
                .program_blockers
                .iter()
                .any(|blocker| blocker.kind == "spindle-activation")
        );
    }

    #[test]
    fn probe_only_readiness_does_not_block_a_program_without_probing() {
        let report = assess_real_run_preflight(
            &program("G21 G90 G94\nG1 X1 F10"),
            hardware(vec![readiness_check(
                "probe-input",
                ReadinessLevel::Blocker,
            )]),
            &snapshot(MachineMode::Idle),
        );

        assert!(report.ready);
        assert_eq!(report.hardware.readiness.blocker_count, 1);
    }

    #[test]
    fn missing_motion_and_non_idle_state_are_independent_blockers() {
        let report = assess_real_run_preflight(
            &program("G21 G90\nM5"),
            hardware(vec![readiness_check(
                "controller-state",
                ReadinessLevel::Blocker,
            )]),
            &snapshot(MachineMode::Run),
        );

        assert!(!report.ready);
        assert_eq!(report.blocker_count, 2);
        assert!(report.checks.iter().any(|item| {
            item.id == "controller-state" && item.level == RunPreflightLevel::Blocker
        }));
        assert!(report.checks.iter().any(|item| {
            item.id == "program-geometry" && item.level == RunPreflightLevel::Blocker
        }));
    }

    #[test]
    fn blocks_motion_that_relies_on_ambient_controller_modes() {
        let report = assess_real_run_preflight(
            &program("G1 X1 F10"),
            hardware(Vec::new()),
            &snapshot(MachineMode::Idle),
        );

        let modal = report
            .checks
            .iter()
            .find(|item| item.id == "program-modal-contract")
            .unwrap();
        assert_eq!(modal.level, RunPreflightLevel::Blocker);
        assert!(modal.detail.contains("G21, G90, G94"));
        assert_eq!(modal.source_line, Some(1));
    }

    #[test]
    fn first_cut_gate_requires_all_six_confirmations_and_fresh_clear_preflight() {
        let now = Instant::now();
        let snapshot = snapshot(MachineMode::Idle);
        let (_, report) = ready_first_cut(&snapshot);
        let mut gate = FirstCutGate::default();
        let mut incomplete = first_cut_confirmation();
        incomplete.safe_z_verified = false;

        assert_eq!(
            gate.authorize(incomplete, &report, &snapshot, now),
            Err(FirstCutAuthorizationError::IncompleteConfirmation {
                missing: vec!["safe Z verified"]
            })
        );

        let mut blocked = report.clone();
        blocked.ready = false;
        blocked.blocker_count = 2;
        assert_eq!(
            gate.authorize(first_cut_confirmation(), &blocked, &snapshot, now),
            Err(FirstCutAuthorizationError::PreflightBlocked { blockers: 2 })
        );

        let mut stale = report.clone();
        stale.poll_sequence -= 1;
        assert_eq!(
            gate.authorize(first_cut_confirmation(), &stale, &snapshot, now),
            Err(FirstCutAuthorizationError::StalePreflightEvidence)
        );

        let mut missing_position = snapshot.clone();
        missing_position.machine.machine_position = None;
        missing_position.machine.work_position = None;
        let (_, report) = ready_first_cut(&missing_position);
        assert_eq!(
            gate.authorize(first_cut_confirmation(), &report, &missing_position, now,),
            Err(FirstCutAuthorizationError::ControllerPositionUnavailable)
        );
    }

    #[test]
    fn first_cut_authorization_is_short_lived_program_bound_and_single_use() {
        let now = Instant::now();
        let snapshot = snapshot(MachineMode::Idle);
        let (_, report) = ready_first_cut(&snapshot);
        let mut gate = FirstCutGate::default();
        let authorization = gate
            .authorize(first_cut_confirmation(), &report, &snapshot, now)
            .unwrap();

        assert_eq!(
            authorization.program_fingerprint,
            report.program_fingerprint
        );
        assert_eq!(authorization.expires_in_ms, 30_000);
        assert_eq!(
            gate.consume(
                authorization.id,
                &program_fingerprint(&program("G21 G90 G94\nG1 X9 F50")),
                &snapshot,
                now,
            ),
            Err(FirstCutAuthorizationError::ProgramChanged)
        );
        assert_eq!(
            gate.consume(
                authorization.id,
                &report.program_fingerprint,
                &snapshot,
                now,
            ),
            Err(FirstCutAuthorizationError::AuthorizationMissing)
        );

        let authorization = gate
            .authorize(first_cut_confirmation(), &report, &snapshot, now)
            .unwrap();
        assert_eq!(
            gate.consume(
                authorization.id,
                &report.program_fingerprint,
                &snapshot,
                now + FIRST_CUT_AUTHORIZATION_TTL + Duration::from_millis(1),
            ),
            Err(FirstCutAuthorizationError::AuthorizationExpired)
        );
    }

    #[test]
    fn first_cut_authorization_dies_when_position_or_controller_session_changes() {
        let now = Instant::now();
        let mut snapshot = snapshot(MachineMode::Idle);
        snapshot.machine.machine_position = Some(Position {
            x: 1.0,
            y: 2.0,
            z: 3.0,
            a: None,
        });
        let (_, report) = ready_first_cut(&snapshot);
        let mut gate = FirstCutGate::default();
        let authorization = gate
            .authorize(first_cut_confirmation(), &report, &snapshot, now)
            .unwrap();

        snapshot.machine.machine_position.as_mut().unwrap().x += 0.01;
        gate.observe(&snapshot, now);
        assert_eq!(
            gate.consume(
                authorization.id,
                &report.program_fingerprint,
                &snapshot,
                now,
            ),
            Err(FirstCutAuthorizationError::AuthorizationMissing)
        );

        snapshot.machine.machine_position.as_mut().unwrap().x = 1.0;
        let (_, report) = ready_first_cut(&snapshot);
        let authorization = gate
            .authorize(first_cut_confirmation(), &report, &snapshot, now)
            .unwrap();
        snapshot.reconnect_count += 1;
        assert_eq!(
            gate.consume(
                authorization.id,
                &report.program_fingerprint,
                &snapshot,
                now,
            ),
            Err(FirstCutAuthorizationError::ControllerSessionChanged)
        );
    }
}
