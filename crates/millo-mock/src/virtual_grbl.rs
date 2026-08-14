use std::{
    collections::VecDeque,
    f64::consts::{PI, TAU},
    time::{Duration, Instant},
};

const DEFAULT_SIMULATION_SPEED: f64 = 1.0;
const INTEGRATION_STEP: Duration = Duration::from_millis(10);
const MIN_SPEED_MM_PER_SEC: f64 = 1e-6;
const MIN_ACCELERATION_MM_PER_SEC2: f64 = 1e-3;

#[derive(Debug, Clone, Copy)]
pub(crate) struct MotionLimits {
    pub(crate) max_rate_mm_per_min: [f64; 3],
    pub(crate) acceleration_mm_per_sec2: [f64; 3],
}

impl MotionLimits {
    fn max_path_rate(self, delta: [f64; 3], length: f64) -> f64 {
        directional_limit(self.max_rate_mm_per_min, delta, length).max(0.001)
    }

    fn path_acceleration(self, delta: [f64; 3], length: f64) -> f64 {
        directional_limit(self.acceleration_mm_per_sec2, delta, length)
            .max(MIN_ACCELERATION_MM_PER_SEC2)
    }

    fn conservative_arc_rate(self) -> f64 {
        self.max_rate_mm_per_min
            .into_iter()
            .filter(|value| value.is_finite() && *value > 0.0)
            .fold(f64::INFINITY, f64::min)
            .max(0.001)
    }

    fn conservative_arc_acceleration(self) -> f64 {
        self.acceleration_mm_per_sec2
            .into_iter()
            .filter(|value| value.is_finite() && *value > 0.0)
            .fold(f64::INFINITY, f64::min)
            .max(MIN_ACCELERATION_MM_PER_SEC2)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MotionMode {
    Rapid,
    Linear,
    ClockwiseArc,
    CounterclockwiseArc,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Plane {
    Xy,
    Xz,
    Yz,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FeedMode {
    UnitsPerMinute,
    InverseTime,
}

#[derive(Debug, Clone)]
struct ModalState {
    motion: MotionMode,
    plane: Plane,
    metric: bool,
    absolute: bool,
    feed_mode: FeedMode,
    feed: f64,
    spindle_rpm: f64,
    spindle_mode: Option<u8>,
    coolant_mist: bool,
    coolant_flood: bool,
    tool: u16,
}

impl Default for ModalState {
    fn default() -> Self {
        Self {
            motion: MotionMode::Rapid,
            plane: Plane::Xy,
            metric: true,
            absolute: true,
            feed_mode: FeedMode::UnitsPerMinute,
            feed: 0.0,
            spindle_rpm: 0.0,
            spindle_mode: None,
            coolant_mist: false,
            coolant_flood: false,
            tool: 0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MotionKind {
    Program { rapid: bool },
    Jog,
}

#[derive(Debug, Clone)]
enum PathGeometry {
    Linear {
        start: [f64; 3],
        end: [f64; 3],
        length: f64,
    },
    Arc {
        start: [f64; 3],
        end: [f64; 3],
        center: [f64; 2],
        axes: [usize; 3],
        radius: f64,
        start_angle: f64,
        sweep: f64,
        length: f64,
    },
}

impl PathGeometry {
    fn linear(start: [f64; 3], end: [f64; 3]) -> Option<Self> {
        let length = distance(start, end);
        (length > f64::EPSILON).then_some(Self::Linear { start, end, length })
    }

    fn length(&self) -> f64 {
        match self {
            Self::Linear { length, .. } | Self::Arc { length, .. } => *length,
        }
    }

    fn end(&self) -> [f64; 3] {
        match self {
            Self::Linear { end, .. } | Self::Arc { end, .. } => *end,
        }
    }

    fn point_at(&self, path_distance: f64) -> [f64; 3] {
        let progress = (path_distance / self.length()).clamp(0.0, 1.0);
        match self {
            Self::Linear { start, end, .. } => interpolate(*start, *end, progress),
            Self::Arc {
                start,
                end,
                center,
                axes: [u, v, w],
                radius,
                start_angle,
                sweep,
                ..
            } => {
                let angle = start_angle + sweep * progress;
                let mut point = *end;
                point[*u] = center[0] + radius * angle.cos();
                point[*v] = center[1] + radius * angle.sin();
                point[*w] = start[*w] + (end[*w] - start[*w]) * progress;
                if progress >= 1.0 { *end } else { point }
            }
        }
    }
}

#[derive(Debug, Clone)]
enum PlannedGeometry {
    Path(PathGeometry),
    Dwell(Duration),
}

#[derive(Debug, Clone)]
struct PlannedMotion {
    geometry: PlannedGeometry,
    maximum_speed_mm_per_sec: f64,
    acceleration_mm_per_sec2: f64,
    source_line: Option<u32>,
    kind: MotionKind,
}

#[derive(Debug, Clone)]
struct ActiveMotion {
    motion: PlannedMotion,
    path_distance_mm: f64,
    speed_mm_per_sec: f64,
    dwell_elapsed: Duration,
}

impl PlannedMotion {
    fn linear_direction(&self) -> Option<[f64; 3]> {
        let PlannedGeometry::Path(PathGeometry::Linear { start, end, length }) = &self.geometry
        else {
            return None;
        };
        Some([
            (end[0] - start[0]) / length,
            (end[1] - start[1]) / length,
            (end[2] - start[2]) / length,
        ])
    }
}

#[derive(Debug, Clone)]
pub(crate) struct VirtualGrbl {
    modal: ModalState,
    position: [f64; 3],
    planned_position: [f64; 3],
    queued: VecDeque<PlannedMotion>,
    active: Option<ActiveMotion>,
    held: bool,
    hold_requested: bool,
    jog_cancel_requested: bool,
    manages_status: bool,
    current_source_line: Option<u32>,
    next_entry_speed_mm_per_sec: f64,
    last_update: Instant,
    simulation_speed: f64,
}

impl VirtualGrbl {
    pub(crate) fn new(position: [f64; 3]) -> Self {
        Self {
            modal: ModalState::default(),
            position,
            planned_position: position,
            queued: VecDeque::new(),
            active: None,
            held: false,
            hold_requested: false,
            jog_cancel_requested: false,
            manages_status: false,
            current_source_line: None,
            next_entry_speed_mm_per_sec: 0.0,
            last_update: Instant::now(),
            simulation_speed: DEFAULT_SIMULATION_SPEED,
        }
    }

    pub(crate) fn execute_line(
        &mut self,
        command: &str,
        work_offsets: &[[f64; 3]; 6],
        active_wcs: &mut usize,
        limits: MotionLimits,
        check_mode: bool,
    ) -> Result<(), u16> {
        let words = parse_words(command)?;
        if words.is_empty() {
            return Ok(());
        }
        let source_line = words
            .iter()
            .find(|word| word.letter == 'N')
            .and_then(|word| valid_source_line(word.value));
        let mut machine_coordinates = false;
        let mut explicit_motion = None;
        let mut dwell_seconds = None;
        let mut program_end = false;

        for word in &words {
            match (word.letter, word.value) {
                ('G', value) if code_is(value, 0.0) => explicit_motion = Some(MotionMode::Rapid),
                ('G', value) if code_is(value, 1.0) => explicit_motion = Some(MotionMode::Linear),
                ('G', value) if code_is(value, 2.0) => {
                    explicit_motion = Some(MotionMode::ClockwiseArc)
                }
                ('G', value) if code_is(value, 3.0) => {
                    explicit_motion = Some(MotionMode::CounterclockwiseArc)
                }
                ('G', value) if code_is(value, 4.0) => {
                    dwell_seconds = word_value(&words, 'P').or_else(|| word_value(&words, 'S'))
                }
                ('G', value) if code_is(value, 17.0) => self.modal.plane = Plane::Xy,
                ('G', value) if code_is(value, 18.0) => self.modal.plane = Plane::Xz,
                ('G', value) if code_is(value, 19.0) => self.modal.plane = Plane::Yz,
                ('G', value) if code_is(value, 20.0) => self.modal.metric = false,
                ('G', value) if code_is(value, 21.0) => self.modal.metric = true,
                ('G', value) if code_is(value, 53.0) => machine_coordinates = true,
                ('G', value) if (54.0..=59.0).contains(&value) && value.fract() == 0.0 => {
                    *active_wcs = (value as usize) - 54;
                }
                ('G', value) if code_is(value, 90.0) => self.modal.absolute = true,
                ('G', value) if code_is(value, 91.0) => self.modal.absolute = false,
                ('G', value) if code_is(value, 93.0) => {
                    self.modal.feed_mode = FeedMode::InverseTime
                }
                ('G', value) if code_is(value, 94.0) => {
                    self.modal.feed_mode = FeedMode::UnitsPerMinute
                }
                ('M', value) if code_is(value, 2.0) || code_is(value, 30.0) => program_end = true,
                ('M', value) if code_is(value, 3.0) => self.modal.spindle_mode = Some(3),
                ('M', value) if code_is(value, 4.0) => self.modal.spindle_mode = Some(4),
                ('M', value) if code_is(value, 5.0) => self.modal.spindle_mode = None,
                ('M', value) if code_is(value, 7.0) => self.modal.coolant_mist = true,
                ('M', value) if code_is(value, 8.0) => self.modal.coolant_flood = true,
                ('M', value) if code_is(value, 9.0) => {
                    self.modal.coolant_mist = false;
                    self.modal.coolant_flood = false;
                }
                ('F', value) if value.is_finite() && value >= 0.0 => {
                    self.modal.feed = value * self.unit_scale()
                }
                ('S', value) if value.is_finite() && value >= 0.0 => self.modal.spindle_rpm = value,
                ('T', value) if value.is_finite() && value >= 0.0 => {
                    self.modal.tool = value.round() as u16
                }
                _ => {}
            }
        }
        if let Some(motion) = explicit_motion {
            self.modal.motion = motion;
        }

        if check_mode {
            if program_end {
                self.reset_modal();
            }
            return Ok(());
        }

        if let Some(seconds) = dwell_seconds.filter(|seconds| *seconds > 0.0) {
            self.enqueue(PlannedMotion {
                geometry: PlannedGeometry::Dwell(Duration::from_secs_f64(seconds)),
                maximum_speed_mm_per_sec: 0.0,
                acceleration_mm_per_sec2: limits.conservative_arc_acceleration(),
                source_line,
                kind: MotionKind::Program { rapid: false },
            });
        }

        if has_axis_words(&words) {
            let start = self.planned_position;
            let target = target_position(
                start,
                &words,
                self.unit_scale(),
                self.modal.absolute,
                machine_coordinates,
                work_offsets[*active_wcs],
            );
            match self.modal.motion {
                MotionMode::Rapid | MotionMode::Linear => {
                    self.enqueue_linear(start, target, source_line, limits);
                }
                MotionMode::ClockwiseArc | MotionMode::CounterclockwiseArc => {
                    self.enqueue_arc(
                        start,
                        target,
                        &words,
                        source_line,
                        self.modal.motion == MotionMode::ClockwiseArc,
                        limits,
                    )?;
                }
            }
            self.planned_position = target;
        }

        if program_end {
            self.modal.spindle_mode = None;
            self.modal.coolant_mist = false;
            self.modal.coolant_flood = false;
            self.reset_modal_motion();
        }
        Ok(())
    }

    pub(crate) fn enqueue_jog(
        &mut self,
        target: [f64; 3],
        feed_mm_per_min: f64,
        limits: MotionLimits,
    ) -> Result<(), u16> {
        if self.active.is_some() || !self.queued.is_empty() || self.held {
            return Err(8);
        }
        let start = self.position;
        let Some(path) = PathGeometry::linear(start, target) else {
            return Ok(());
        };
        let delta = subtract(target, start);
        let length = path.length();
        let maximum_rate = feed_mm_per_min
            .max(0.001)
            .min(limits.max_path_rate(delta, length));
        self.planned_position = target;
        self.enqueue(PlannedMotion {
            geometry: PlannedGeometry::Path(path),
            maximum_speed_mm_per_sec: maximum_rate / 60.0,
            acceleration_mm_per_sec2: limits.path_acceleration(delta, length),
            source_line: None,
            kind: MotionKind::Jog,
        });
        Ok(())
    }

    pub(crate) fn status_line(
        &mut self,
        work_offset: [f64; 3],
        overrides: [u16; 3],
    ) -> Option<String> {
        if !self.manages_status {
            return None;
        }
        let now = Instant::now();
        let elapsed = now.saturating_duration_since(self.last_update);
        self.last_update = now;
        self.advance(elapsed, overrides);
        let running = self.active.is_some() || !self.queued.is_empty();
        let mode = if self.held {
            "Hold:0"
        } else if self.hold_requested {
            "Hold:1"
        } else if self.active_kind() == Some(MotionKind::Jog) {
            "Jog"
        } else if running {
            "Run"
        } else {
            "Idle"
        };
        let feed = self
            .active
            .as_ref()
            .map_or(0.0, |active| active.speed_mm_per_sec * 60.0);
        let work = subtract(self.position, work_offset);
        let line = self
            .active
            .as_ref()
            .and_then(|active| active.motion.source_line)
            .or(self.current_source_line);
        let used_planner_blocks = usize::from(self.active.is_some()) + self.queued.len();
        let available_planner_blocks = 15_usize.saturating_sub(used_planner_blocks.min(15));
        let mut fields = vec![
            mode.to_owned(),
            format_position("MPos", self.position),
            format_position("WPos", work),
            format!(
                "FS:{feed:.3},{:.0}",
                if self.modal.spindle_mode.is_some() {
                    self.modal.spindle_rpm
                } else {
                    0.0
                }
            ),
            format!("Bf:{available_planner_blocks},128"),
            format!("Ov:{},{},{}", overrides[0], overrides[1], overrides[2]),
        ];
        if let Some(line) = line {
            fields.push(format!("Ln:{line}"));
        }
        let accessories = self.accessories();
        if !accessories.is_empty() {
            fields.push(format!("A:{accessories}"));
        }
        let status = format!("<{}>", fields.join("|"));
        if !running && !self.held && !self.hold_requested {
            self.manages_status = false;
        }
        Some(status)
    }

    pub(crate) fn modal_report(&self, active_wcs: usize) -> String {
        let motion = match self.modal.motion {
            MotionMode::Rapid => "G0",
            MotionMode::Linear => "G1",
            MotionMode::ClockwiseArc => "G2",
            MotionMode::CounterclockwiseArc => "G3",
        };
        let plane = match self.modal.plane {
            Plane::Xy => "G17",
            Plane::Xz => "G18",
            Plane::Yz => "G19",
        };
        let units = if self.modal.metric { "G21" } else { "G20" };
        let distance = if self.modal.absolute { "G90" } else { "G91" };
        let feed_mode = match self.modal.feed_mode {
            FeedMode::UnitsPerMinute => "G94",
            FeedMode::InverseTime => "G93",
        };
        let spindle = self
            .modal
            .spindle_mode
            .map_or("M5".to_owned(), |mode| format!("M{mode}"));
        let coolant = if self.modal.coolant_flood {
            "M8"
        } else if self.modal.coolant_mist {
            "M7"
        } else {
            "M9"
        };
        format!(
            "[GC:{motion} G{} {plane} {units} {distance} {feed_mode} {spindle} {coolant} T{} F{:.3} S{:.0}]",
            active_wcs + 54,
            self.modal.tool,
            self.modal.feed,
            self.modal.spindle_rpm
        )
    }

    pub(crate) fn hold(&mut self, overrides: [u16; 3]) {
        self.advance_from_clock(overrides);
        if self.active.is_some() || !self.queued.is_empty() {
            self.hold_requested = true;
            self.manages_status = true;
        }
    }

    pub(crate) fn resume(&mut self) {
        if self.held || self.hold_requested {
            self.held = false;
            self.hold_requested = false;
            self.last_update = Instant::now();
            self.manages_status = self.active.is_some() || !self.queued.is_empty();
        }
    }

    pub(crate) fn cancel_jog(&mut self, overrides: [u16; 3]) {
        self.advance_from_clock(overrides);
        if self.active_kind() == Some(MotionKind::Jog)
            || self
                .queued
                .iter()
                .any(|motion| motion.kind == MotionKind::Jog)
        {
            self.jog_cancel_requested = true;
            self.held = false;
            self.hold_requested = false;
            self.manages_status = true;
        }
    }

    pub(crate) fn stop(&mut self, position: [f64; 3]) {
        self.queued.clear();
        self.active = None;
        self.held = false;
        self.hold_requested = false;
        self.jog_cancel_requested = false;
        self.position = position;
        self.planned_position = position;
        self.current_source_line = None;
        self.next_entry_speed_mm_per_sec = 0.0;
        self.manages_status = false;
        self.next_entry_speed_mm_per_sec = 0.0;
        self.last_update = Instant::now();
        self.reset_modal();
    }

    pub(crate) fn sync_external_position(&mut self, position: [f64; 3]) {
        self.position = position;
        self.planned_position = position;
        self.queued.clear();
        self.active = None;
        self.held = false;
        self.hold_requested = false;
        self.jog_cancel_requested = false;
        self.manages_status = false;
        self.last_update = Instant::now();
    }

    pub(crate) fn advance_for_test(&mut self, elapsed: Duration, overrides: [u16; 3]) {
        self.advance(elapsed, overrides);
    }

    fn enqueue_linear(
        &mut self,
        start: [f64; 3],
        end: [f64; 3],
        source_line: Option<u32>,
        limits: MotionLimits,
    ) {
        let Some(path) = PathGeometry::linear(start, end) else {
            return;
        };
        let delta = subtract(end, start);
        let length = path.length();
        let axis_rate = limits.max_path_rate(delta, length);
        let requested_rate = if self.modal.motion == MotionMode::Rapid {
            axis_rate
        } else {
            match self.modal.feed_mode {
                FeedMode::UnitsPerMinute => self.modal.feed.max(0.001),
                FeedMode::InverseTime => length * self.modal.feed.max(0.001),
            }
        };
        self.enqueue(PlannedMotion {
            geometry: PlannedGeometry::Path(path),
            maximum_speed_mm_per_sec: requested_rate.min(axis_rate) / 60.0,
            acceleration_mm_per_sec2: limits.path_acceleration(delta, length),
            source_line,
            kind: MotionKind::Program {
                rapid: self.modal.motion == MotionMode::Rapid,
            },
        });
    }

    fn enqueue_arc(
        &mut self,
        start: [f64; 3],
        end: [f64; 3],
        words: &[Word],
        source_line: Option<u32>,
        clockwise: bool,
        limits: MotionLimits,
    ) -> Result<(), u16> {
        let [u, v, w] = match self.modal.plane {
            Plane::Xy => [0, 1, 2],
            Plane::Xz => [0, 2, 1],
            Plane::Yz => [1, 2, 0],
        };
        let [center_u_letter, center_v_letter] = match self.modal.plane {
            Plane::Xy => ['I', 'J'],
            Plane::Xz => ['I', 'K'],
            Plane::Yz => ['J', 'K'],
        };
        let scale = self.unit_scale();
        let start_uv = [start[u], start[v]];
        let end_uv = [end[u], end[v]];
        let center = if let Some(radius) = word_value(words, 'R').map(|value| value * scale) {
            radius_center(start_uv, end_uv, radius, clockwise).ok_or(33_u16)?
        } else {
            [
                start_uv[0] + word_value(words, center_u_letter).unwrap_or(0.0) * scale,
                start_uv[1] + word_value(words, center_v_letter).unwrap_or(0.0) * scale,
            ]
        };
        let radius = (start_uv[0] - center[0]).hypot(start_uv[1] - center[1]);
        if !radius.is_finite() || radius <= f64::EPSILON {
            return Err(33);
        }
        let start_angle = (start_uv[1] - center[1]).atan2(start_uv[0] - center[0]);
        let end_angle = (end_uv[1] - center[1]).atan2(end_uv[0] - center[0]);
        let full_circle = distance_2d(start_uv, end_uv) <= 1e-9;
        let sweep = directed_sweep(start_angle, end_angle, clockwise, full_circle);
        let planar_length = radius * sweep.abs();
        let length = planar_length.hypot(end[w] - start[w]);
        if !length.is_finite() || length <= f64::EPSILON {
            return Err(33);
        }
        let requested_rate = match self.modal.feed_mode {
            FeedMode::UnitsPerMinute => self.modal.feed.max(0.001),
            FeedMode::InverseTime => length * self.modal.feed.max(0.001),
        };
        let maximum_rate = requested_rate.min(limits.conservative_arc_rate());
        self.enqueue(PlannedMotion {
            geometry: PlannedGeometry::Path(PathGeometry::Arc {
                start,
                end,
                center,
                axes: [u, v, w],
                radius,
                start_angle,
                sweep,
                length,
            }),
            maximum_speed_mm_per_sec: maximum_rate / 60.0,
            acceleration_mm_per_sec2: limits.conservative_arc_acceleration(),
            source_line,
            kind: MotionKind::Program { rapid: false },
        });
        Ok(())
    }

    fn enqueue(&mut self, motion: PlannedMotion) {
        self.queued.push_back(motion);
        self.manages_status = true;
        self.last_update = Instant::now();
    }

    fn active_kind(&self) -> Option<MotionKind> {
        self.active
            .as_ref()
            .map(|active| active.motion.kind)
            .or_else(|| self.queued.front().map(|motion| motion.kind))
    }

    fn advance_from_clock(&mut self, overrides: [u16; 3]) {
        let now = Instant::now();
        let elapsed = now.saturating_duration_since(self.last_update);
        self.last_update = now;
        self.advance(elapsed, overrides);
    }

    fn advance(&mut self, elapsed: Duration, overrides: [u16; 3]) {
        let mut budget = elapsed.mul_f64(self.simulation_speed);
        while !budget.is_zero() && !self.held {
            if self.active.is_none() {
                if self.hold_requested {
                    self.held = true;
                    self.hold_requested = false;
                    break;
                }
                let Some(motion) = self.queued.pop_front() else {
                    break;
                };
                self.current_source_line = motion.source_line;
                self.active = Some(ActiveMotion {
                    speed_mm_per_sec: self
                        .next_entry_speed_mm_per_sec
                        .min(motion.maximum_speed_mm_per_sec),
                    motion,
                    path_distance_mm: 0.0,
                    dwell_elapsed: Duration::ZERO,
                });
                self.next_entry_speed_mm_per_sec = 0.0;
            }

            let step = budget.min(INTEGRATION_STEP);
            budget -= step;
            if self.advance_active(step, overrides) {
                let exit_speed = self
                    .active
                    .as_ref()
                    .map_or(0.0, |active| active.speed_mm_per_sec);
                self.next_entry_speed_mm_per_sec = self
                    .active
                    .as_ref()
                    .and_then(|active| {
                        self.queued
                            .front()
                            .map(|next| junction_speed(&active.motion, next, exit_speed))
                    })
                    .unwrap_or(0.0);
                self.active = None;
                if self.jog_cancel_requested {
                    self.queued.retain(|motion| motion.kind != MotionKind::Jog);
                    self.jog_cancel_requested = false;
                    self.planned_position = self.position;
                }
            }
        }
    }

    fn advance_active(&mut self, step: Duration, overrides: [u16; 3]) -> bool {
        let active = self.active.as_mut().expect("active motion initialized");
        match &active.motion.geometry {
            PlannedGeometry::Dwell(duration) => {
                if self.hold_requested {
                    self.held = true;
                    self.hold_requested = false;
                    return false;
                }
                active.dwell_elapsed += step;
                active.dwell_elapsed >= *duration
            }
            PlannedGeometry::Path(path) => {
                let total_length = path.length();
                let remaining = (total_length - active.path_distance_mm).max(0.0);
                let acceleration = active
                    .motion
                    .acceleration_mm_per_sec2
                    .max(MIN_ACCELERATION_MM_PER_SEC2);
                let stopping = self.hold_requested || self.jog_cancel_requested;
                let override_factor = match active.motion.kind {
                    MotionKind::Program { rapid: true } => f64::from(overrides[1]) / 100.0,
                    MotionKind::Program { rapid: false } => f64::from(overrides[0]) / 100.0,
                    MotionKind::Jog => 1.0,
                };
                let programmed_limit = active.motion.maximum_speed_mm_per_sec * override_factor;
                let exit_speed = if stopping {
                    0.0
                } else {
                    self.queued.front().map_or(0.0, |next| {
                        junction_speed(&active.motion, next, programmed_limit)
                    })
                };
                let braking_limit = (exit_speed.powi(2) + 2.0 * acceleration * remaining).sqrt();
                let desired_speed = if stopping {
                    0.0
                } else {
                    programmed_limit.min(braking_limit)
                };
                let seconds = step.as_secs_f64();
                let previous_speed = active.speed_mm_per_sec;
                active.speed_mm_per_sec =
                    approach(previous_speed, desired_speed, acceleration * seconds);
                let travelled = (previous_speed + active.speed_mm_per_sec) * 0.5 * seconds;
                active.path_distance_mm = (active.path_distance_mm + travelled).min(total_length);
                self.position = path.point_at(active.path_distance_mm);

                if stopping && active.speed_mm_per_sec <= MIN_SPEED_MM_PER_SEC {
                    if self.jog_cancel_requested {
                        return true;
                    }
                    self.held = true;
                    self.hold_requested = false;
                    return false;
                }
                if active.path_distance_mm >= total_length - 1e-9 {
                    self.position = path.end();
                    return true;
                }
                false
            }
        }
    }

    fn unit_scale(&self) -> f64 {
        if self.modal.metric { 1.0 } else { 25.4 }
    }

    fn accessories(&self) -> String {
        let mut value = String::new();
        if self.modal.spindle_mode.is_some() {
            value.push('S');
        }
        if self.modal.coolant_flood {
            value.push('F');
        }
        if self.modal.coolant_mist {
            value.push('M');
        }
        value
    }

    fn reset_modal_motion(&mut self) {
        self.modal.motion = MotionMode::Rapid;
        self.modal.plane = Plane::Xy;
        self.modal.metric = true;
        self.modal.absolute = true;
        self.modal.feed_mode = FeedMode::UnitsPerMinute;
    }

    fn reset_modal(&mut self) {
        self.modal = ModalState::default();
    }
}

#[derive(Debug, Clone, Copy)]
struct Word {
    letter: char,
    value: f64,
}

fn parse_words(command: &str) -> Result<Vec<Word>, u16> {
    if !command.is_ascii() {
        return Err(2);
    }
    let bytes = command.as_bytes();
    let mut words = Vec::new();
    let mut cursor = 0;
    while cursor < bytes.len() {
        match bytes[cursor] {
            byte if byte.is_ascii_whitespace() || byte == b'%' => {
                cursor += 1;
                continue;
            }
            b';' | b'*' => break,
            b'(' => {
                cursor += 1;
                while cursor < bytes.len() && bytes[cursor] != b')' {
                    cursor += 1;
                }
                if cursor == bytes.len() {
                    return Err(2);
                }
                cursor += 1;
                continue;
            }
            byte if byte.is_ascii_alphabetic() => {}
            _ => return Err(2),
        }

        let letter = char::from(bytes[cursor]).to_ascii_uppercase();
        cursor += 1;
        let value_start = cursor;
        if cursor < bytes.len() && matches!(bytes[cursor], b'+' | b'-') {
            cursor += 1;
        }
        let mut digit_seen = false;
        let mut decimal_seen = false;
        while cursor < bytes.len() {
            match bytes[cursor] {
                byte if byte.is_ascii_digit() => {
                    digit_seen = true;
                    cursor += 1;
                }
                b'.' if !decimal_seen => {
                    decimal_seen = true;
                    cursor += 1;
                }
                _ => break,
            }
        }
        if !digit_seen {
            return Err(2);
        }
        let value = command[value_start..cursor]
            .parse::<f64>()
            .map_err(|_| 2_u16)?;
        words.push(Word { letter, value });
    }
    Ok(words)
}

fn valid_source_line(value: f64) -> Option<u32> {
    (value.is_finite() && value >= 0.0 && value <= u32::MAX as f64).then(|| value.round() as u32)
}

fn code_is(value: f64, expected: f64) -> bool {
    (value - expected).abs() <= 1e-6
}

fn word_value(words: &[Word], letter: char) -> Option<f64> {
    words
        .iter()
        .rev()
        .find(|word| word.letter == letter)
        .map(|word| word.value)
}

fn has_axis_words(words: &[Word]) -> bool {
    words
        .iter()
        .any(|word| matches!(word.letter, 'X' | 'Y' | 'Z'))
}

fn target_position(
    start: [f64; 3],
    words: &[Word],
    scale: f64,
    absolute: bool,
    machine_coordinates: bool,
    work_offset: [f64; 3],
) -> [f64; 3] {
    let mut target = start;
    for (axis, letter) in ['X', 'Y', 'Z'].into_iter().enumerate() {
        let Some(value) = word_value(words, letter) else {
            continue;
        };
        let value = value * scale;
        target[axis] = if absolute {
            value
                + if machine_coordinates {
                    0.0
                } else {
                    work_offset[axis]
                }
        } else {
            start[axis] + value
        };
    }
    target
}

fn directional_limit(axis_limits: [f64; 3], delta: [f64; 3], length: f64) -> f64 {
    delta
        .into_iter()
        .zip(axis_limits)
        .filter_map(|(component, limit)| {
            let ratio = component.abs() / length;
            (ratio > f64::EPSILON && limit.is_finite() && limit > 0.0).then_some(limit / ratio)
        })
        .fold(f64::INFINITY, f64::min)
}

fn radius_center(
    start: [f64; 2],
    end: [f64; 2],
    signed_radius: f64,
    clockwise: bool,
) -> Option<[f64; 2]> {
    let chord = distance_2d(start, end);
    let radius = signed_radius.abs();
    if chord <= f64::EPSILON || radius < chord / 2.0 {
        return None;
    }
    let midpoint = [(start[0] + end[0]) / 2.0, (start[1] + end[1]) / 2.0];
    let height = (radius * radius - chord * chord / 4.0).max(0.0).sqrt();
    let perpendicular = [-(end[1] - start[1]) / chord, (end[0] - start[0]) / chord];
    let candidates = [
        [
            midpoint[0] + perpendicular[0] * height,
            midpoint[1] + perpendicular[1] * height,
        ],
        [
            midpoint[0] - perpendicular[0] * height,
            midpoint[1] - perpendicular[1] * height,
        ],
    ];
    candidates.into_iter().find(|center| {
        let start_angle = (start[1] - center[1]).atan2(start[0] - center[0]);
        let end_angle = (end[1] - center[1]).atan2(end[0] - center[0]);
        let sweep = directed_sweep(start_angle, end_angle, clockwise, false).abs();
        if signed_radius >= 0.0 {
            sweep <= PI + 1e-9
        } else {
            sweep >= PI - 1e-9
        }
    })
}

fn directed_sweep(start: f64, end: f64, clockwise: bool, full_circle: bool) -> f64 {
    if full_circle {
        return if clockwise { -TAU } else { TAU };
    }
    if clockwise {
        -((start - end).rem_euclid(TAU))
    } else {
        (end - start).rem_euclid(TAU)
    }
}

fn distance(left: [f64; 3], right: [f64; 3]) -> f64 {
    ((right[0] - left[0]).powi(2) + (right[1] - left[1]).powi(2) + (right[2] - left[2]).powi(2))
        .sqrt()
}

fn junction_speed(current: &PlannedMotion, next: &PlannedMotion, candidate: f64) -> f64 {
    if current.kind != next.kind || matches!(current.kind, MotionKind::Jog) {
        return 0.0;
    }
    let (Some(current_direction), Some(next_direction)) =
        (current.linear_direction(), next.linear_direction())
    else {
        return 0.0;
    };
    let alignment = current_direction
        .into_iter()
        .zip(next_direction)
        .map(|(left, right)| left * right)
        .sum::<f64>();
    if alignment < 0.999_999 {
        return 0.0;
    }
    candidate
        .min(current.maximum_speed_mm_per_sec)
        .min(next.maximum_speed_mm_per_sec)
}

fn distance_2d(left: [f64; 2], right: [f64; 2]) -> f64 {
    (right[0] - left[0]).hypot(right[1] - left[1])
}

fn interpolate(start: [f64; 3], end: [f64; 3], progress: f64) -> [f64; 3] {
    [
        start[0] + (end[0] - start[0]) * progress,
        start[1] + (end[1] - start[1]) * progress,
        start[2] + (end[2] - start[2]) * progress,
    ]
}

fn subtract(left: [f64; 3], right: [f64; 3]) -> [f64; 3] {
    [left[0] - right[0], left[1] - right[1], left[2] - right[2]]
}

fn approach(current: f64, target: f64, maximum_change: f64) -> f64 {
    if current < target {
        (current + maximum_change).min(target)
    } else {
        (current - maximum_change).max(target)
    }
}

fn format_position(name: &str, position: [f64; 3]) -> String {
    format!(
        "{name}:{:.3},{:.3},{:.3}",
        position[0], position[1], position[2]
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn limits() -> MotionLimits {
        MotionLimits {
            max_rate_mm_per_min: [1_000.0, 1_000.0, 500.0],
            acceleration_mm_per_sec2: [50.0, 50.0, 30.0],
        }
    }

    fn finish(machine: &mut VirtualGrbl) {
        machine.advance_for_test(Duration::from_secs(3_600), [100; 3]);
    }

    #[test]
    fn executes_absolute_and_incremental_program_motion() {
        let mut machine = VirtualGrbl::new([0.0; 3]);
        let offsets = [[0.0; 3]; 6];
        let mut wcs = 0;

        machine
            .execute_line(
                "N1 G21 G90 G94 G1 X10 Y5 Z-1 F300",
                &offsets,
                &mut wcs,
                limits(),
                false,
            )
            .unwrap();
        machine
            .execute_line("N2 G91 X2 Y-1", &offsets, &mut wcs, limits(), false)
            .unwrap();
        finish(&mut machine);

        assert_eq!(machine.position, [12.0, 4.0, -1.0]);
        assert_eq!(machine.current_source_line, Some(2));
    }

    #[test]
    fn acceleration_and_braking_are_visible_in_intermediate_status() {
        let mut machine = VirtualGrbl::new([0.0; 3]);
        let offsets = [[0.0; 3]; 6];
        let mut wcs = 0;
        machine
            .execute_line(
                "N5 G21 G90 G94 G1 X100 F1000",
                &offsets,
                &mut wcs,
                limits(),
                false,
            )
            .unwrap();

        machine.advance_for_test(Duration::from_millis(100), [100; 3]);
        let accelerating_speed = machine.active.as_ref().unwrap().speed_mm_per_sec;
        machine.advance_for_test(Duration::from_secs(2), [100; 3]);
        let cruising_speed = machine.active.as_ref().unwrap().speed_mm_per_sec;
        assert!(accelerating_speed > 0.0 && accelerating_speed < cruising_speed);

        machine.advance_for_test(Duration::from_millis(4_100), [100; 3]);
        let braking_speed = machine.active.as_ref().unwrap().speed_mm_per_sec;
        assert!(braking_speed < cruising_speed);
        finish(&mut machine);
        assert_eq!(machine.position, [100.0, 0.0, 0.0]);
    }

    #[test]
    fn collinear_program_blocks_cross_the_source_line_without_a_false_stop() {
        let mut machine = VirtualGrbl::new([0.0; 3]);
        let offsets = [[0.0; 3]; 6];
        let mut wcs = 0;
        machine
            .execute_line(
                "N1 G21 G90 G94 G1 X10 F600",
                &offsets,
                &mut wcs,
                limits(),
                false,
            )
            .unwrap();
        machine
            .execute_line("N2 G1 X20 F600", &offsets, &mut wcs, limits(), false)
            .unwrap();

        machine.advance_for_test(Duration::from_millis(1_150), [100; 3]);

        assert!(machine.position[0] > 10.0);
        assert!(machine.active.as_ref().unwrap().speed_mm_per_sec > 9.0);
        finish(&mut machine);
        assert_eq!(machine.position, [20.0, 0.0, 0.0]);
    }

    #[test]
    fn interpolates_arcs_without_segment_stops() {
        let mut machine = VirtualGrbl::new([0.0; 3]);
        let offsets = [[0.0; 3]; 6];
        let mut wcs = 0;
        machine
            .execute_line(
                "N1 G21 G90 G94 G1 X10 F300",
                &offsets,
                &mut wcs,
                limits(),
                false,
            )
            .unwrap();
        finish(&mut machine);
        machine
            .execute_line(
                "N2 G17 G3 X20 Y10 I0 J10 F300",
                &offsets,
                &mut wcs,
                limits(),
                false,
            )
            .unwrap();
        machine.advance_for_test(Duration::from_millis(500), [100; 3]);

        assert!(machine.position[0] > 10.0 && machine.position[0] < 20.0);
        assert!(machine.position[1] > 0.0 && machine.position[1] < 10.0);
        finish(&mut machine);
        assert_eq!(machine.position, [20.0, 10.0, 0.0]);
    }

    #[test]
    fn hold_decelerates_before_freezing_and_resume_accelerates_again() {
        let mut machine = VirtualGrbl::new([0.0; 3]);
        let offsets = [[0.0; 3]; 6];
        let mut wcs = 0;
        machine
            .execute_line(
                "N7 G21 G90 G94 G1 X100 F1000",
                &offsets,
                &mut wcs,
                limits(),
                false,
            )
            .unwrap();
        machine.advance_for_test(Duration::from_secs(1), [100; 3]);
        machine.hold([100; 3]);
        let requested_at = machine.position[0];
        machine.advance_for_test(Duration::from_millis(100), [100; 3]);
        assert!(machine.position[0] > requested_at);
        machine.advance_for_test(Duration::from_secs(1), [100; 3]);
        assert!(machine.held);
        let held_at = machine.position;
        machine.advance_for_test(Duration::from_secs(10), [100; 3]);
        assert_eq!(machine.position, held_at);

        machine.resume();
        finish(&mut machine);
        assert_eq!(machine.position, [100.0, 0.0, 0.0]);
    }

    #[test]
    fn check_mode_updates_modal_state_without_moving() {
        let mut machine = VirtualGrbl::new([1.0, 2.0, 3.0]);
        let offsets = [[0.0; 3]; 6];
        let mut wcs = 0;
        machine
            .execute_line(
                "N9 G21 G90 G94 G1 X50 F200 S12000 M3",
                &offsets,
                &mut wcs,
                limits(),
                true,
            )
            .unwrap();

        assert_eq!(machine.position, [1.0, 2.0, 3.0]);
        assert!(
            machine
                .modal_report(wcs)
                .contains("G1 G54 G17 G21 G90 G94 M3")
        );
    }

    #[test]
    fn accepts_compact_words_and_inline_comments_like_grbl() {
        let mut machine = VirtualGrbl::new([0.0; 3]);
        let offsets = [[0.0; 3]; 6];
        let mut wcs = 0;

        machine
            .execute_line(
                "N12G21G90G1X3.5(comment)Y-2F120; ignored",
                &offsets,
                &mut wcs,
                limits(),
                false,
            )
            .unwrap();
        finish(&mut machine);

        assert_eq!(machine.position, [3.5, -2.0, 0.0]);
        assert_eq!(machine.current_source_line, Some(12));
    }

    #[test]
    fn inverse_time_linear_feed_is_a_block_duration_before_acceleration_limits() {
        let mut machine = VirtualGrbl::new([0.0; 3]);
        let offsets = [[0.0; 3]; 6];
        let mut wcs = 0;

        machine
            .execute_line(
                "N3 G21 G90 G93 G1 X100 F1",
                &offsets,
                &mut wcs,
                limits(),
                false,
            )
            .unwrap();
        machine.advance_for_test(Duration::from_secs(30), [100; 3]);

        assert!(machine.position[0] > 0.0 && machine.position[0] < 100.0);
        finish(&mut machine);
        assert_eq!(machine.position, [100.0, 0.0, 0.0]);
    }
}
