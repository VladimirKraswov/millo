use std::time::{Duration, Instant};

use millo_domain::{
    ConnectionState, ControllerSnapshot, HardwareInspection, MachineMode, OperatorConfirmation,
    ResetChallenge, TestJogAuthorization,
};
use thiserror::Error;

pub const RESET_CHALLENGE_TTL: Duration = Duration::from_secs(10);
pub const TEST_JOG_AUTHORIZATION_TTL: Duration = Duration::from_secs(15);

#[derive(Debug, Error, PartialEq, Eq)]
pub enum SafetyError {
    #[error("operator confirmation is incomplete")]
    IncompleteOperatorConfirmation,
    #[error("hardware readiness contains {blockers} blocker(s)")]
    ReadinessBlocked { blockers: usize },
    #[error("controller is not connected and idle")]
    UnsafeControllerState,
    #[error("soft reset confirmation is missing")]
    ResetChallengeMissing,
    #[error("soft reset confirmation does not match the active challenge")]
    ResetChallengeMismatch,
    #[error("soft reset confirmation expired")]
    ResetChallengeExpired,
    #[error("test jog authorization is missing")]
    TestJogAuthorizationMissing,
    #[error("test jog authorization does not match the active lease")]
    TestJogAuthorizationMismatch,
    #[error("test jog authorization expired")]
    TestJogAuthorizationExpired,
    #[error("controller session changed after test jog authorization")]
    ControllerSessionChanged,
}

#[derive(Debug)]
struct ResetLease {
    id: u64,
    expires_at: Instant,
}

#[derive(Debug)]
struct TestJogLease {
    authorization: TestJogAuthorization,
    expires_at: Instant,
    reset_count: u64,
    reconnect_count: u32,
}

#[derive(Debug, Default)]
pub struct SafetyManager {
    next_id: u64,
    reset: Option<ResetLease>,
    test_jog: Option<TestJogLease>,
}

impl SafetyManager {
    pub fn request_soft_reset(&mut self, now: Instant) -> ResetChallenge {
        let id = self.allocate_id();
        self.reset = Some(ResetLease {
            id,
            expires_at: now + RESET_CHALLENGE_TTL,
        });
        ResetChallenge {
            id,
            expires_in_ms: duration_ms(RESET_CHALLENGE_TTL),
        }
    }

    pub fn confirm_soft_reset(&mut self, id: u64, now: Instant) -> Result<(), SafetyError> {
        let Some(challenge) = self.reset.take() else {
            return Err(SafetyError::ResetChallengeMissing);
        };
        if challenge.id != id {
            return Err(SafetyError::ResetChallengeMismatch);
        }
        if now > challenge.expires_at {
            return Err(SafetyError::ResetChallengeExpired);
        }
        self.test_jog = None;
        Ok(())
    }

    pub fn authorize_test_jog(
        &mut self,
        confirmation: OperatorConfirmation,
        inspection: &HardwareInspection,
        snapshot: &ControllerSnapshot,
        now: Instant,
    ) -> Result<TestJogAuthorization, SafetyError> {
        if !confirmation.is_complete() {
            return Err(SafetyError::IncompleteOperatorConfirmation);
        }
        if !inspection.readiness.test_jog_ready {
            return Err(SafetyError::ReadinessBlocked {
                blockers: inspection.readiness.blocker_count,
            });
        }
        if !stable_idle(snapshot) {
            return Err(SafetyError::UnsafeControllerState);
        }

        let authorization = TestJogAuthorization {
            id: self.allocate_id(),
            expires_in_ms: duration_ms(TEST_JOG_AUTHORIZATION_TTL),
        };
        self.test_jog = Some(TestJogLease {
            authorization,
            expires_at: now + TEST_JOG_AUTHORIZATION_TTL,
            reset_count: snapshot.reset_count,
            reconnect_count: snapshot.reconnect_count,
        });
        Ok(authorization)
    }

    pub fn observe(&mut self, snapshot: &ControllerSnapshot, now: Instant) {
        if self.test_jog_is_valid(snapshot, now).is_err() {
            self.test_jog = None;
        }
        if self
            .reset
            .as_ref()
            .is_some_and(|challenge| now > challenge.expires_at)
        {
            self.reset = None;
        }
    }

    pub fn consume_test_jog(
        &mut self,
        id: u64,
        snapshot: &ControllerSnapshot,
        now: Instant,
    ) -> Result<(), SafetyError> {
        let validation = self.test_jog_is_valid(snapshot, now);
        let Some(lease) = self.test_jog.take() else {
            return Err(validation
                .err()
                .unwrap_or(SafetyError::TestJogAuthorizationMissing));
        };
        validation?;
        if lease.authorization.id != id {
            return Err(SafetyError::TestJogAuthorizationMismatch);
        }
        Ok(())
    }

    pub fn invalidate_test_jog(&mut self) {
        self.test_jog = None;
    }

    fn test_jog_is_valid(
        &self,
        snapshot: &ControllerSnapshot,
        now: Instant,
    ) -> Result<(), SafetyError> {
        let Some(lease) = self.test_jog.as_ref() else {
            return Err(SafetyError::TestJogAuthorizationMissing);
        };
        if now > lease.expires_at {
            return Err(SafetyError::TestJogAuthorizationExpired);
        }
        if lease.reset_count != snapshot.reset_count
            || lease.reconnect_count != snapshot.reconnect_count
        {
            return Err(SafetyError::ControllerSessionChanged);
        }
        if !stable_idle(snapshot) {
            return Err(SafetyError::UnsafeControllerState);
        }
        Ok(())
    }

    fn allocate_id(&mut self) -> u64 {
        self.next_id = self.next_id.saturating_add(1);
        self.next_id
    }
}

fn stable_idle(snapshot: &ControllerSnapshot) -> bool {
    snapshot.connection == ConnectionState::Connected
        && snapshot.machine.mode == MachineMode::Idle
        && snapshot.alarm.is_none()
        && snapshot.reset_notice.is_none()
}

fn duration_ms(duration: Duration) -> u64 {
    duration.as_millis().try_into().unwrap_or(u64::MAX)
}

#[cfg(test)]
mod tests {
    use millo_domain::{
        DeviceInspection, HardwareProfile, MachineState, ReadinessReport, SpindleControl,
    };

    use super::*;

    fn idle_snapshot() -> ControllerSnapshot {
        ControllerSnapshot {
            connection: ConnectionState::Connected,
            machine: MachineState {
                mode: MachineMode::Idle,
                reported_mode: "Idle".to_owned(),
                ..Default::default()
            },
            ..Default::default()
        }
    }

    fn ready_inspection() -> HardwareInspection {
        HardwareInspection {
            device: DeviceInspection::default(),
            readiness: ReadinessReport {
                profile: HardwareProfile {
                    name: "test".to_owned(),
                    axes: vec!["X".to_owned(), "Y".to_owned(), "Z".to_owned()],
                    travel_mm: None,
                    max_jog_distance_mm: 50.0,
                    spindle_control: SpindleControl::Manual,
                    homing_installed: false,
                    limit_switches_installed: false,
                    probe_installed: false,
                    emergency_stop_installed: false,
                },
                test_jog_ready: true,
                probe_ready: false,
                blocker_count: 0,
                caution_count: 0,
                checks: Vec::new(),
            },
        }
    }

    fn confirmation() -> OperatorConfirmation {
        OperatorConfirmation {
            spindle_off: true,
            tool_clear: true,
            power_control_reachable: true,
        }
    }

    #[test]
    fn reset_requires_the_current_unexpired_challenge() {
        let now = Instant::now();
        let mut safety = SafetyManager::default();
        let challenge = safety.request_soft_reset(now);

        assert_eq!(
            safety.confirm_soft_reset(challenge.id + 1, now),
            Err(SafetyError::ResetChallengeMismatch)
        );
        assert_eq!(
            safety.confirm_soft_reset(challenge.id, now),
            Err(SafetyError::ResetChallengeMissing)
        );

        let expired = safety.request_soft_reset(now);
        assert_eq!(
            safety.confirm_soft_reset(
                expired.id,
                now + RESET_CHALLENGE_TTL + Duration::from_millis(1)
            ),
            Err(SafetyError::ResetChallengeExpired)
        );
    }

    #[test]
    fn test_jog_requires_every_operator_confirmation() {
        let mut safety = SafetyManager::default();
        let mut incomplete = confirmation();
        incomplete.spindle_off = false;

        let result = safety.authorize_test_jog(
            incomplete,
            &ready_inspection(),
            &idle_snapshot(),
            Instant::now(),
        );

        assert_eq!(result, Err(SafetyError::IncompleteOperatorConfirmation));
    }

    #[test]
    fn readiness_blockers_prevent_authorization() {
        let mut safety = SafetyManager::default();
        let mut inspection = ready_inspection();
        inspection.readiness.test_jog_ready = false;
        inspection.readiness.blocker_count = 2;

        let result = safety.authorize_test_jog(
            confirmation(),
            &inspection,
            &idle_snapshot(),
            Instant::now(),
        );

        assert_eq!(result, Err(SafetyError::ReadinessBlocked { blockers: 2 }));
    }

    #[test]
    fn authorization_is_short_lived_and_single_use() {
        let now = Instant::now();
        let snapshot = idle_snapshot();
        let mut safety = SafetyManager::default();
        let authorization = safety
            .authorize_test_jog(confirmation(), &ready_inspection(), &snapshot, now)
            .unwrap();

        safety
            .consume_test_jog(authorization.id, &snapshot, now)
            .unwrap();
        assert_eq!(
            safety.consume_test_jog(authorization.id, &snapshot, now),
            Err(SafetyError::TestJogAuthorizationMissing)
        );

        let expired = safety
            .authorize_test_jog(confirmation(), &ready_inspection(), &snapshot, now)
            .unwrap();
        assert_eq!(
            safety.consume_test_jog(
                expired.id,
                &snapshot,
                now + TEST_JOG_AUTHORIZATION_TTL + Duration::from_millis(1)
            ),
            Err(SafetyError::TestJogAuthorizationExpired)
        );
    }

    #[test]
    fn alarm_reset_or_reconnect_invalidates_authorization() {
        let now = Instant::now();
        let snapshot = idle_snapshot();
        let mut safety = SafetyManager::default();
        safety
            .authorize_test_jog(confirmation(), &ready_inspection(), &snapshot, now)
            .unwrap();

        let mut alarm = snapshot.clone();
        alarm.machine.mode = MachineMode::Alarm;
        safety.observe(&alarm, now);
        assert_eq!(
            safety.consume_test_jog(1, &snapshot, now),
            Err(SafetyError::TestJogAuthorizationMissing)
        );

        let authorization = safety
            .authorize_test_jog(confirmation(), &ready_inspection(), &snapshot, now)
            .unwrap();
        let mut reconnected = snapshot.clone();
        reconnected.reconnect_count = 1;
        assert_eq!(
            safety.consume_test_jog(authorization.id, &reconnected, now),
            Err(SafetyError::ControllerSessionChanged)
        );
    }
}
