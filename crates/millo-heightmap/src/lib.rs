use std::{
    fs, io,
    path::{Path, PathBuf},
};

use millo_storage::{backup_path, write_atomically};
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const HEIGHTMAP_SCHEMA_VERSION: u16 = 1;
pub const MAX_GRID_AXIS_POINTS: usize = 101;
pub const MAX_GRID_POINTS: usize = 10_000;
pub const MAX_ABSOLUTE_COORDINATE_MM: f64 = 100_000.0;
pub const SURFACE_SESSION_SCHEMA_VERSION: u16 = 1;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum HeightmapContactMode {
    #[default]
    DirectSurface,
    FixedPlate,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HeightmapPlanRequest {
    pub origin_x_mm: f64,
    pub origin_y_mm: f64,
    pub width_mm: f64,
    pub height_mm: f64,
    pub columns: usize,
    pub rows: usize,
    pub clearance_z_mm: f64,
    pub max_probe_depth_mm: f64,
    pub probe_feed_mm_per_min: f64,
    pub travel_feed_mm_per_min: f64,
    pub retract_feed_mm_per_min: f64,
    pub contact_mode: HeightmapContactMode,
    pub contact_offset_mm: f64,
}

impl Default for HeightmapPlanRequest {
    fn default() -> Self {
        Self {
            origin_x_mm: 0.0,
            origin_y_mm: 0.0,
            width_mm: 50.0,
            height_mm: 50.0,
            columns: 6,
            rows: 6,
            clearance_z_mm: 2.0,
            max_probe_depth_mm: 3.0,
            probe_feed_mm_per_min: 25.0,
            travel_feed_mm_per_min: 300.0,
            retract_feed_mm_per_min: 100.0,
            contact_mode: HeightmapContactMode::DirectSurface,
            contact_offset_mm: 0.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HeightmapTravelLimits {
    pub x_mm: f64,
    pub y_mm: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HeightmapSpacing {
    pub x_mm: f64,
    pub y_mm: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProbePoint {
    pub sequence: usize,
    pub row: usize,
    pub column: usize,
    pub x_mm: f64,
    pub y_mm: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HeightmapPlan {
    pub schema_version: u16,
    pub request: HeightmapPlanRequest,
    pub spacing: HeightmapSpacing,
    pub points: Vec<ProbePoint>,
}

impl HeightmapPlan {
    /// Conservative duration estimate. Actual probing is usually faster because
    /// contact occurs before the configured maximum search depth.
    pub fn estimated_max_seconds(&self) -> f64 {
        let request = self.request;
        let xy_distance = self
            .points
            .windows(2)
            .map(|points| {
                let dx = points[1].x_mm - points[0].x_mm;
                let dy = points[1].y_mm - points[0].y_mm;
                dx.hypot(dy)
            })
            .sum::<f64>();
        let point_count = self.points.len() as f64;
        let probe_seconds = request.max_probe_depth_mm / request.probe_feed_mm_per_min * 60.0;
        let retract_seconds = (request.clearance_z_mm + request.max_probe_depth_mm)
            / request.retract_feed_mm_per_min
            * 60.0;
        xy_distance / request.travel_feed_mm_per_min * 60.0
            + point_count * (probe_seconds + retract_seconds)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProbeSample {
    pub point: ProbePoint,
    pub z_mm: f64,
    pub triggered: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HeightmapProgress {
    pub measured: usize,
    pub triggered: usize,
    pub total: usize,
    pub complete: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Heightmap {
    pub schema_version: u16,
    pub plan: HeightmapPlan,
    pub samples: Vec<Option<ProbeSample>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HeightmapStartRequest {
    pub plan: HeightmapPlanRequest,
    pub setup_confirmed: bool,
    pub contact_available_at_every_point: bool,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum HeightmapOperationState {
    #[default]
    Idle,
    Running,
    Paused,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HeightmapOperationSnapshot {
    pub operation_sequence: u64,
    pub state: HeightmapOperationState,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub map: Option<Heightmap>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_sequence: Option<usize>,
    pub progress: HeightmapProgress,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl Default for HeightmapOperationSnapshot {
    fn default() -> Self {
        Self {
            operation_sequence: 0,
            state: HeightmapOperationState::Idle,
            map: None,
            current_sequence: None,
            progress: HeightmapProgress {
                measured: 0,
                triggered: 0,
                total: 0,
                complete: false,
            },
            error: None,
        }
    }
}

impl HeightmapOperationSnapshot {
    pub fn running(operation_sequence: u64, map: Heightmap) -> Self {
        let progress = map.progress();
        Self {
            operation_sequence,
            state: HeightmapOperationState::Running,
            current_sequence: map.plan.points.first().map(|point| point.sequence),
            map: Some(map),
            progress,
            error: None,
        }
    }

    pub fn observe_map(&mut self, map: Heightmap, current_sequence: Option<usize>) {
        self.progress = map.progress();
        self.map = Some(map);
        self.current_sequence = current_sequence;
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StoredSurfaceMap {
    pub map_id: u64,
    pub machine_profile_id: String,
    pub created_at_unix_ms: u64,
    pub map: Heightmap,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PendingSurfaceMap {
    pub machine_profile_id: String,
    pub updated_at_unix_ms: u64,
    pub operation: HeightmapOperationSnapshot,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SurfaceSession {
    pub schema_version: u16,
    pub revision: u64,
    pub next_map_id: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active: Option<StoredSurfaceMap>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pending: Option<PendingSurfaceMap>,
    pub application_enabled: bool,
    pub requires_setup_confirmation: bool,
}

impl Default for SurfaceSession {
    fn default() -> Self {
        Self {
            schema_version: SURFACE_SESSION_SCHEMA_VERSION,
            revision: 0,
            next_map_id: 1,
            active: None,
            pending: None,
            application_enabled: false,
            requires_setup_confirmation: false,
        }
    }
}

#[derive(Debug)]
pub struct SurfaceSessionStore {
    path: Option<PathBuf>,
    session: SurfaceSession,
}

impl SurfaceSessionStore {
    pub fn in_memory() -> Self {
        Self {
            path: None,
            session: SurfaceSession::default(),
        }
    }

    pub fn load(path: impl Into<PathBuf>) -> Result<Self, SurfaceSessionError> {
        let path = path.into();
        let backup = backup_path(&path);
        if !path.exists() && !backup.exists() {
            return Ok(Self {
                path: Some(path),
                session: SurfaceSession::default(),
            });
        }
        let (mut session, recovered) = if path.exists() {
            match read_surface_session(&path) {
                Ok(session) => (session, false),
                Err(SurfaceSessionError::InvalidFile(primary)) if backup.exists() => {
                    match read_surface_session(&backup) {
                        Ok(session) => (session, true),
                        Err(SurfaceSessionError::InvalidFile(backup)) => {
                            return Err(SurfaceSessionError::CorruptCopies { primary, backup });
                        }
                        Err(error) => return Err(error),
                    }
                }
                Err(error) => return Err(error),
            }
        } else {
            (read_surface_session(&backup)?, true)
        };
        // Map data survives restart, but compensation never silently re-arms
        // after the workpiece may have moved.
        let restart_flags_changed = session.active.is_some()
            && (session.application_enabled || !session.requires_setup_confirmation);
        if session.active.is_some() {
            session.application_enabled = false;
            session.requires_setup_confirmation = true;
        }
        if recovered || restart_flags_changed {
            let _ = fs::remove_file(&path);
            save_surface_session(&path, &session)?;
        }
        Ok(Self {
            path: Some(path),
            session,
        })
    }

    pub fn session(&self) -> SurfaceSession {
        self.session.clone()
    }

    pub fn begin(
        &mut self,
        machine_profile_id: impl Into<String>,
        operation: HeightmapOperationSnapshot,
        now_unix_ms: u64,
    ) -> Result<SurfaceSession, SurfaceSessionError> {
        if operation.state != HeightmapOperationState::Running || operation.map.is_none() {
            return Err(SurfaceSessionError::InvalidOperation(
                "pending map must start in Running state",
            ));
        }
        let mut next = self.session.clone();
        next.pending = Some(PendingSurfaceMap {
            machine_profile_id: machine_profile_id.into(),
            updated_at_unix_ms: now_unix_ms,
            operation,
        });
        next.revision = next.revision.saturating_add(1);
        self.commit(next)
    }

    pub fn checkpoint(
        &mut self,
        operation: HeightmapOperationSnapshot,
        now_unix_ms: u64,
    ) -> Result<SurfaceSession, SurfaceSessionError> {
        let mut next = self.session.clone();
        let pending = next
            .pending
            .as_mut()
            .ok_or(SurfaceSessionError::NoPendingMap)?;
        if pending.operation.operation_sequence != operation.operation_sequence {
            return Err(SurfaceSessionError::OperationMismatch);
        }
        pending.operation = operation;
        pending.updated_at_unix_ms = now_unix_ms;
        next.revision = next.revision.saturating_add(1);
        self.commit(next)
    }

    pub fn activate_completed(
        &mut self,
        operation_sequence: u64,
        now_unix_ms: u64,
    ) -> Result<SurfaceSession, SurfaceSessionError> {
        let mut next = self.session.clone();
        let pending = next
            .pending
            .take()
            .ok_or(SurfaceSessionError::NoPendingMap)?;
        if pending.operation.operation_sequence != operation_sequence {
            return Err(SurfaceSessionError::OperationMismatch);
        }
        let map = pending.operation.map.ok_or(SurfaceSessionError::NoMap)?;
        let progress = map.progress();
        if !progress.complete || progress.triggered != progress.total {
            return Err(SurfaceSessionError::IncompleteMap);
        }
        next.active = Some(StoredSurfaceMap {
            map_id: next.next_map_id,
            machine_profile_id: pending.machine_profile_id,
            created_at_unix_ms: now_unix_ms,
            map,
        });
        next.next_map_id = next.next_map_id.saturating_add(1);
        next.application_enabled = false;
        next.requires_setup_confirmation = true;
        next.revision = next.revision.saturating_add(1);
        self.commit(next)
    }

    pub fn set_application_enabled(
        &mut self,
        enabled: bool,
        setup_confirmed: bool,
    ) -> Result<SurfaceSession, SurfaceSessionError> {
        let mut next = self.session.clone();
        if enabled {
            if next.active.is_none() {
                return Err(SurfaceSessionError::NoMap);
            }
            if !setup_confirmed {
                return Err(SurfaceSessionError::SetupConfirmationRequired);
            }
        }
        next.application_enabled = enabled;
        next.requires_setup_confirmation = false;
        next.revision = next.revision.saturating_add(1);
        self.commit(next)
    }

    pub fn discard_pending(&mut self) -> Result<SurfaceSession, SurfaceSessionError> {
        let mut next = self.session.clone();
        next.pending = None;
        next.revision = next.revision.saturating_add(1);
        self.commit(next)
    }

    pub fn forget_active(&mut self) -> Result<SurfaceSession, SurfaceSessionError> {
        let mut next = self.session.clone();
        next.active = None;
        next.application_enabled = false;
        next.requires_setup_confirmation = false;
        next.revision = next.revision.saturating_add(1);
        self.commit(next)
    }

    fn commit(&mut self, next: SurfaceSession) -> Result<SurfaceSession, SurfaceSessionError> {
        if let Some(path) = &self.path {
            save_surface_session(path, &next)?;
        }
        self.session = next;
        Ok(self.session())
    }
}

#[derive(Debug, Error)]
pub enum SurfaceSessionError {
    #[error("invalid surface-session operation: {0}")]
    InvalidOperation(&'static str),
    #[error("there is no pending heightmap")]
    NoPendingMap,
    #[error("heightmap operation does not match the pending operation")]
    OperationMismatch,
    #[error("there is no completed heightmap")]
    NoMap,
    #[error("heightmap must be complete and every point must have contact")]
    IncompleteMap,
    #[error("confirm that the workpiece and work zero have not moved before applying the map")]
    SetupConfirmationRequired,
    #[error("unsupported surface-session schema version: {0}")]
    UnsupportedSchema(u16),
    #[error("invalid surface-session file: {0}")]
    InvalidFile(serde_json::Error),
    #[error("surface-session primary and backup are corrupt: primary: {primary}; backup: {backup}")]
    CorruptCopies {
        primary: serde_json::Error,
        backup: serde_json::Error,
    },
    #[error("surface-session I/O failed for {path}: {source}")]
    Io { path: PathBuf, source: io::Error },
}

fn read_surface_session(path: &Path) -> Result<SurfaceSession, SurfaceSessionError> {
    let bytes = fs::read(path).map_err(|source| SurfaceSessionError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    let session = serde_json::from_slice::<SurfaceSession>(&bytes)
        .map_err(SurfaceSessionError::InvalidFile)?;
    if session.schema_version != SURFACE_SESSION_SCHEMA_VERSION {
        return Err(SurfaceSessionError::UnsupportedSchema(
            session.schema_version,
        ));
    }
    Ok(session)
}

fn save_surface_session(path: &Path, session: &SurfaceSession) -> Result<(), SurfaceSessionError> {
    let bytes = serde_json::to_vec_pretty(session).map_err(SurfaceSessionError::InvalidFile)?;
    write_atomically(path, &bytes).map_err(|source| SurfaceSessionError::Io {
        path: path.to_path_buf(),
        source,
    })
}

#[derive(Debug, Error, PartialEq)]
pub enum HeightmapError {
    #[error("invalid heightmap setting: {0}")]
    InvalidSetting(&'static str),
    #[error("heightmap grid requires at least 2 points on each axis")]
    GridTooSmall,
    #[error("heightmap grid supports at most {MAX_GRID_AXIS_POINTS} points on each axis")]
    GridTooLarge,
    #[error("heightmap grid exceeds the {MAX_GRID_POINTS}-point limit")]
    TooManyPoints,
    #[error("heightmap {axis} span {requested:.3} mm exceeds machine travel {maximum:.3} mm")]
    ExceedsTravel {
        axis: &'static str,
        requested: f64,
        maximum: f64,
    },
    #[error("heightmap point {0} does not exist")]
    UnknownPoint(usize),
    #[error("probe sample Z must be finite")]
    InvalidSample,
    #[error("heightmap is incomplete")]
    Incomplete,
    #[error("heightmap contains a probe miss at point {0}")]
    ProbeMiss(usize),
    #[error("sample coordinate is outside the heightmap area")]
    OutsideArea,
}

pub fn plan_heightmap(
    request: HeightmapPlanRequest,
    travel: Option<HeightmapTravelLimits>,
) -> Result<HeightmapPlan, HeightmapError> {
    validate_request(request, travel)?;
    let spacing = HeightmapSpacing {
        x_mm: request.width_mm / (request.columns - 1) as f64,
        y_mm: request.height_mm / (request.rows - 1) as f64,
    };
    let total = request
        .columns
        .checked_mul(request.rows)
        .ok_or(HeightmapError::TooManyPoints)?;
    let mut points = Vec::with_capacity(total);
    for row in 0..request.rows {
        let columns: Box<dyn Iterator<Item = usize>> = if row % 2 == 0 {
            Box::new(0..request.columns)
        } else {
            Box::new((0..request.columns).rev())
        };
        for column in columns {
            points.push(ProbePoint {
                sequence: points.len(),
                row,
                column,
                x_mm: request.origin_x_mm + spacing.x_mm * column as f64,
                y_mm: request.origin_y_mm + spacing.y_mm * row as f64,
            });
        }
    }
    Ok(HeightmapPlan {
        schema_version: HEIGHTMAP_SCHEMA_VERSION,
        request,
        spacing,
        points,
    })
}

fn validate_request(
    request: HeightmapPlanRequest,
    travel: Option<HeightmapTravelLimits>,
) -> Result<(), HeightmapError> {
    for (name, value) in [
        ("origin X", request.origin_x_mm),
        ("origin Y", request.origin_y_mm),
        ("width", request.width_mm),
        ("height", request.height_mm),
        ("clearance Z", request.clearance_z_mm),
        ("maximum probe depth", request.max_probe_depth_mm),
        ("probe feed", request.probe_feed_mm_per_min),
        ("travel feed", request.travel_feed_mm_per_min),
        ("retract feed", request.retract_feed_mm_per_min),
        ("contact offset", request.contact_offset_mm),
    ] {
        if !value.is_finite() {
            return Err(HeightmapError::InvalidSetting(name));
        }
    }
    if request.width_mm <= 0.0 {
        return Err(HeightmapError::InvalidSetting("width"));
    }
    if request.height_mm <= 0.0 {
        return Err(HeightmapError::InvalidSetting("height"));
    }
    let end_x = request.origin_x_mm + request.width_mm;
    let end_y = request.origin_y_mm + request.height_mm;
    if request.origin_x_mm.abs() > MAX_ABSOLUTE_COORDINATE_MM
        || end_x.abs() > MAX_ABSOLUTE_COORDINATE_MM
    {
        return Err(HeightmapError::InvalidSetting("X perimeter"));
    }
    if request.origin_y_mm.abs() > MAX_ABSOLUTE_COORDINATE_MM
        || end_y.abs() > MAX_ABSOLUTE_COORDINATE_MM
    {
        return Err(HeightmapError::InvalidSetting("Y perimeter"));
    }
    if request.clearance_z_mm <= 0.0 {
        return Err(HeightmapError::InvalidSetting("clearance Z"));
    }
    if request.max_probe_depth_mm <= 0.0 {
        return Err(HeightmapError::InvalidSetting("maximum probe depth"));
    }
    if request.probe_feed_mm_per_min <= 0.0 || request.probe_feed_mm_per_min > 1_000.0 {
        return Err(HeightmapError::InvalidSetting("probe feed"));
    }
    if request.travel_feed_mm_per_min < 10.0 || request.travel_feed_mm_per_min > 100_000.0 {
        return Err(HeightmapError::InvalidSetting("travel feed"));
    }
    if request.retract_feed_mm_per_min < 10.0 || request.retract_feed_mm_per_min > 100_000.0 {
        return Err(HeightmapError::InvalidSetting("retract feed"));
    }
    match request.contact_mode {
        HeightmapContactMode::DirectSurface if request.contact_offset_mm != 0.0 => {
            return Err(HeightmapError::InvalidSetting(
                "direct contact offset must be zero",
            ));
        }
        HeightmapContactMode::FixedPlate
            if !(0.01..=100.0).contains(&request.contact_offset_mm) =>
        {
            return Err(HeightmapError::InvalidSetting("contact plate thickness"));
        }
        _ => {}
    }
    if request.columns < 2 || request.rows < 2 {
        return Err(HeightmapError::GridTooSmall);
    }
    if request.columns > MAX_GRID_AXIS_POINTS || request.rows > MAX_GRID_AXIS_POINTS {
        return Err(HeightmapError::GridTooLarge);
    }
    if request
        .columns
        .checked_mul(request.rows)
        .is_none_or(|total| total > MAX_GRID_POINTS)
    {
        return Err(HeightmapError::TooManyPoints);
    }
    if let Some(travel) = travel {
        if !travel.x_mm.is_finite() || travel.x_mm <= 0.0 {
            return Err(HeightmapError::InvalidSetting("machine X travel"));
        }
        if !travel.y_mm.is_finite() || travel.y_mm <= 0.0 {
            return Err(HeightmapError::InvalidSetting("machine Y travel"));
        }
        if request.width_mm > travel.x_mm {
            return Err(HeightmapError::ExceedsTravel {
                axis: "X",
                requested: request.width_mm,
                maximum: travel.x_mm,
            });
        }
        if request.height_mm > travel.y_mm {
            return Err(HeightmapError::ExceedsTravel {
                axis: "Y",
                requested: request.height_mm,
                maximum: travel.y_mm,
            });
        }
    }
    Ok(())
}

impl Heightmap {
    pub fn new(plan: HeightmapPlan) -> Self {
        let point_count = plan.points.len();
        Self {
            schema_version: HEIGHTMAP_SCHEMA_VERSION,
            plan,
            samples: vec![None; point_count],
        }
    }

    pub fn record_sample(
        &mut self,
        sequence: usize,
        z_mm: f64,
        triggered: bool,
    ) -> Result<HeightmapProgress, HeightmapError> {
        if !z_mm.is_finite() {
            return Err(HeightmapError::InvalidSample);
        }
        let point = *self
            .plan
            .points
            .get(sequence)
            .ok_or(HeightmapError::UnknownPoint(sequence))?;
        self.samples[sequence] = Some(ProbeSample {
            point,
            z_mm,
            triggered,
        });
        Ok(self.progress())
    }

    pub fn progress(&self) -> HeightmapProgress {
        let measured = self
            .samples
            .iter()
            .filter(|sample| sample.is_some())
            .count();
        let triggered = self
            .samples
            .iter()
            .filter(|sample| sample.is_some_and(|sample| sample.triggered))
            .count();
        HeightmapProgress {
            measured,
            triggered,
            total: self.samples.len(),
            complete: measured == self.samples.len(),
        }
    }

    pub fn z_range(&self) -> Option<(f64, f64)> {
        let mut values = self
            .samples
            .iter()
            .filter_map(|sample| sample.as_ref())
            .filter(|sample| sample.triggered)
            .map(|sample| sample.z_mm);
        let first = values.next()?;
        Some(values.fold((first, first), |(minimum, maximum), value| {
            (minimum.min(value), maximum.max(value))
        }))
    }

    pub fn interpolate_z(&self, x_mm: f64, y_mm: f64) -> Result<f64, HeightmapError> {
        if !x_mm.is_finite() || !y_mm.is_finite() {
            return Err(HeightmapError::OutsideArea);
        }
        let request = self.plan.request;
        let x_offset = x_mm - request.origin_x_mm;
        let y_offset = y_mm - request.origin_y_mm;
        const EPSILON: f64 = 1e-9;
        if x_offset < -EPSILON
            || y_offset < -EPSILON
            || x_offset > request.width_mm + EPSILON
            || y_offset > request.height_mm + EPSILON
        {
            return Err(HeightmapError::OutsideArea);
        }

        let grid_x = (x_offset / self.plan.spacing.x_mm).clamp(0.0, (request.columns - 1) as f64);
        let grid_y = (y_offset / self.plan.spacing.y_mm).clamp(0.0, (request.rows - 1) as f64);
        let left = grid_x.floor() as usize;
        let bottom = grid_y.floor() as usize;
        let right = (left + 1).min(request.columns - 1);
        let top = (bottom + 1).min(request.rows - 1);
        let x_mix = grid_x - left as f64;
        let y_mix = grid_y - bottom as f64;

        let z00 = self.sample_at(bottom, left)?;
        let z10 = self.sample_at(bottom, right)?;
        let z01 = self.sample_at(top, left)?;
        let z11 = self.sample_at(top, right)?;
        let bottom_z = z00 + (z10 - z00) * x_mix;
        let top_z = z01 + (z11 - z01) * x_mix;
        Ok(bottom_z + (top_z - bottom_z) * y_mix)
    }

    fn sample_at(&self, row: usize, column: usize) -> Result<f64, HeightmapError> {
        let columns = self.plan.request.columns;
        let sequence = if row % 2 == 0 {
            row * columns + column
        } else {
            row * columns + (columns - 1 - column)
        };
        let sample = self
            .samples
            .get(sequence)
            .and_then(Option::as_ref)
            .ok_or(HeightmapError::Incomplete)?;
        if !sample.triggered {
            return Err(HeightmapError::ProbeMiss(sequence));
        }
        Ok(sample.z_mm)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static TEST_ID: AtomicU64 = AtomicU64::new(1);

    fn plan(columns: usize, rows: usize) -> HeightmapPlan {
        plan_heightmap(
            HeightmapPlanRequest {
                width_mm: 20.0,
                height_mm: 10.0,
                columns,
                rows,
                ..HeightmapPlanRequest::default()
            },
            Some(HeightmapTravelLimits {
                x_mm: 500.0,
                y_mm: 500.0,
            }),
        )
        .unwrap()
    }

    #[test]
    fn builds_a_serpentine_grid_without_duplicate_points() {
        let plan = plan(3, 3);
        let coordinates = plan
            .points
            .iter()
            .map(|point| (point.row, point.column, point.x_mm, point.y_mm))
            .collect::<Vec<_>>();
        assert_eq!(
            coordinates,
            vec![
                (0, 0, 0.0, 0.0),
                (0, 1, 10.0, 0.0),
                (0, 2, 20.0, 0.0),
                (1, 2, 20.0, 5.0),
                (1, 1, 10.0, 5.0),
                (1, 0, 0.0, 5.0),
                (2, 0, 0.0, 10.0),
                (2, 1, 10.0, 10.0),
                (2, 2, 20.0, 10.0),
            ]
        );
    }

    #[test]
    fn rejects_unsafe_or_unbounded_plans() {
        let request = HeightmapPlanRequest {
            columns: 1,
            ..HeightmapPlanRequest::default()
        };
        assert_eq!(
            plan_heightmap(request, None),
            Err(HeightmapError::GridTooSmall)
        );

        let request = HeightmapPlanRequest {
            width_mm: 501.0,
            ..HeightmapPlanRequest::default()
        };
        assert_eq!(
            plan_heightmap(
                request,
                Some(HeightmapTravelLimits {
                    x_mm: 500.0,
                    y_mm: 500.0,
                })
            ),
            Err(HeightmapError::ExceedsTravel {
                axis: "X",
                requested: 501.0,
                maximum: 500.0,
            })
        );
    }

    #[test]
    fn records_progress_without_double_counting_replaced_samples() {
        let mut map = Heightmap::new(plan(2, 2));
        assert_eq!(map.record_sample(0, -0.1, true).unwrap().measured, 1);
        let replaced = map.record_sample(0, -0.2, true).unwrap();
        assert_eq!(replaced.measured, 1);
        assert_eq!(replaced.triggered, 1);
        assert!(!replaced.complete);
        assert_eq!(map.z_range(), Some((-0.2, -0.2)));
    }

    #[test]
    fn interpolates_a_plane_across_serpentine_storage() {
        let mut map = Heightmap::new(plan(3, 3));
        for point in map.plan.points.clone() {
            let z = 1.0 + point.x_mm * 0.1 + point.y_mm * 0.2;
            map.record_sample(point.sequence, z, true).unwrap();
        }
        assert!((map.interpolate_z(5.0, 2.5).unwrap() - 2.0).abs() < 1e-9);
        assert!((map.interpolate_z(20.0, 10.0).unwrap() - 5.0).abs() < 1e-9);
    }

    #[test]
    fn refuses_incomplete_maps_and_probe_misses() {
        let mut map = Heightmap::new(plan(2, 2));
        map.record_sample(0, 0.0, true).unwrap();
        assert_eq!(map.interpolate_z(2.0, 2.0), Err(HeightmapError::Incomplete));

        for sequence in 0..4 {
            map.record_sample(sequence, sequence as f64, sequence != 3)
                .unwrap();
        }
        assert_eq!(
            map.interpolate_z(0.0, 10.0),
            Err(HeightmapError::ProbeMiss(3))
        );
    }

    #[test]
    fn schema_round_trips_through_json() {
        let mut map = Heightmap::new(plan(2, 2));
        map.record_sample(0, -0.25, true).unwrap();
        let json = serde_json::to_string(&map).unwrap();
        let restored: Heightmap = serde_json::from_str(&json).unwrap();
        assert_eq!(restored, map);
        assert_eq!(restored.schema_version, HEIGHTMAP_SCHEMA_VERSION);
    }

    #[test]
    fn operation_snapshot_tracks_progress_without_losing_the_map() {
        let mut map = Heightmap::new(plan(2, 2));
        let mut snapshot = HeightmapOperationSnapshot::running(7, map.clone());
        map.record_sample(0, -0.125, true).unwrap();
        snapshot.observe_map(map, Some(1));

        assert_eq!(snapshot.operation_sequence, 7);
        assert_eq!(snapshot.state, HeightmapOperationState::Running);
        assert_eq!(snapshot.current_sequence, Some(1));
        assert_eq!(snapshot.progress.measured, 1);
        assert_eq!(snapshot.map.unwrap().samples[0].unwrap().z_mm, -0.125);
    }

    #[test]
    fn contact_mode_never_silently_reuses_a_touch_plate_offset() {
        let direct = HeightmapPlanRequest {
            contact_offset_mm: 19.1,
            ..HeightmapPlanRequest::default()
        };
        assert_eq!(
            plan_heightmap(direct, None),
            Err(HeightmapError::InvalidSetting(
                "direct contact offset must be zero"
            ))
        );

        let plate = HeightmapPlanRequest {
            contact_mode: HeightmapContactMode::FixedPlate,
            contact_offset_mm: 19.1,
            ..HeightmapPlanRequest::default()
        };
        assert!(plan_heightmap(plate, None).is_ok());
    }

    #[test]
    fn reports_grid_spacing_and_a_conservative_duration() {
        let plan = plan(3, 3);
        assert_eq!(
            plan.spacing,
            HeightmapSpacing {
                x_mm: 10.0,
                y_mm: 5.0
            }
        );
        assert!((plan.estimated_max_seconds() - 105.8).abs() < 1e-9);
    }

    #[test]
    fn rejects_unbounded_absolute_perimeters() {
        let request = HeightmapPlanRequest {
            origin_x_mm: 99_990.0,
            width_mm: 20.0,
            ..HeightmapPlanRequest::default()
        };
        assert_eq!(
            plan_heightmap(request, None),
            Err(HeightmapError::InvalidSetting("X perimeter"))
        );
    }

    fn completed_operation(sequence: u64, z_offset: f64) -> HeightmapOperationSnapshot {
        let mut map = Heightmap::new(plan(2, 2));
        for point in map.plan.points.clone() {
            map.record_sample(
                point.sequence,
                z_offset + point.sequence as f64 * 0.01,
                true,
            )
            .unwrap();
        }
        let mut snapshot = HeightmapOperationSnapshot::running(sequence, map.clone());
        snapshot.state = HeightmapOperationState::Completed;
        snapshot.observe_map(map, None);
        snapshot
    }

    fn session_path() -> PathBuf {
        std::env::temp_dir().join(format!(
            "millo-surface-session-{}-{}.json",
            std::process::id(),
            TEST_ID.fetch_add(1, Ordering::Relaxed)
        ))
    }

    #[test]
    fn incomplete_replacement_never_destroys_the_active_map() {
        let mut store = SurfaceSessionStore::in_memory();
        let running = HeightmapOperationSnapshot::running(1, Heightmap::new(plan(2, 2)));
        store.begin("machine-1", running, 10).unwrap();
        store.checkpoint(completed_operation(1, 0.1), 11).unwrap();
        store.activate_completed(1, 12).unwrap();
        let active = store.session().active.unwrap();

        let next = HeightmapOperationSnapshot::running(2, Heightmap::new(plan(3, 3)));
        store.begin("machine-1", next, 20).unwrap();
        store.discard_pending().unwrap();

        assert_eq!(store.session().active.unwrap(), active);
    }

    #[test]
    fn completed_replacement_is_atomic_and_gets_a_new_identity() {
        let mut store = SurfaceSessionStore::in_memory();
        for (sequence, z) in [(1, 0.1), (2, 0.2)] {
            store
                .begin(
                    "machine-1",
                    HeightmapOperationSnapshot::running(sequence, Heightmap::new(plan(2, 2))),
                    sequence * 10,
                )
                .unwrap();
            store
                .checkpoint(completed_operation(sequence, z), sequence * 10 + 1)
                .unwrap();
            store
                .activate_completed(sequence, sequence * 10 + 2)
                .unwrap();
        }
        let active = store.session().active.unwrap();
        assert_eq!(active.map_id, 2);
        assert_eq!(active.map.samples[0].unwrap().z_mm, 0.2);
    }

    #[test]
    fn restart_restores_data_but_requires_fixture_confirmation_before_application() {
        let path = session_path();
        let mut store = SurfaceSessionStore::load(&path).unwrap();
        store
            .begin(
                "machine-1",
                HeightmapOperationSnapshot::running(1, Heightmap::new(plan(2, 2))),
                10,
            )
            .unwrap();
        store.checkpoint(completed_operation(1, 0.1), 11).unwrap();
        store.activate_completed(1, 12).unwrap();
        store.set_application_enabled(true, true).unwrap();

        let restored = SurfaceSessionStore::load(&path).unwrap().session();
        assert!(restored.active.is_some());
        assert!(!restored.application_enabled);
        assert!(restored.requires_setup_confirmation);

        let persisted = SurfaceSessionStore::load(&path).unwrap().session();
        assert!(!persisted.application_enabled);
        assert!(persisted.requires_setup_confirmation);

        let _ = fs::remove_file(&path);
        let _ = fs::remove_file(backup_path(&path));
    }
}
