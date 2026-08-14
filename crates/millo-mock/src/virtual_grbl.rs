use std::{
    collections::VecDeque,
    f64::consts::{PI, TAU},
    time::{Duration, Instant},
};

const DEFAULT_SIMULATION_SPEED: f64 = 20.0;
const MIN_SIMULATED_MOTION: Duration = Duration::from_millis(20);
const MAX_SIMULATED_MOTION: Duration = Duration::from_secs(2);
const ARC_CHORD_MM: f64 = 0.5;
const MAX_ARC_SEGMENTS: usize = 720;

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

#[derive(Debug, Clone)]
struct PlannedMotion {
    end: [f64; 3],
    duration: Duration,
    source_line: Option<u32>,
    feed_mm_per_min: f64,
}

#[derive(Debug, Clone)]
struct ActiveMotion {
    start: [f64; 3],
    motion: PlannedMotion,
    elapsed: Duration,
}

#[derive(Debug, Clone)]
pub(crate) struct VirtualGrbl {
    modal: ModalState,
    position: [f64; 3],
    planned_position: [f64; 3],
    queued: VecDeque<PlannedMotion>,
    active: Option<ActiveMotion>,
    held: bool,
    manages_status: bool,
    current_source_line: Option<u32>,
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
            manages_status: false,
            current_source_line: None,
            last_update: Instant::now(),
            simulation_speed: DEFAULT_SIMULATION_SPEED,
        }
    }

    pub(crate) fn execute_line(
        &mut self,
        command: &str,
        work_offsets: &[[f64; 3]; 6],
        active_wcs: &mut usize,
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
            self.enqueue_motion(PlannedMotion {
                end: self.planned_position,
                duration: self.simulated_duration(seconds),
                source_line,
                feed_mm_per_min: 0.0,
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
                    let feed = if self.modal.motion == MotionMode::Rapid {
                        2_000.0
                    } else {
                        self.modal.feed.max(0.001)
                    };
                    self.enqueue_linear(start, target, source_line, feed);
                }
                MotionMode::ClockwiseArc | MotionMode::CounterclockwiseArc => {
                    self.enqueue_arc(
                        start,
                        target,
                        &words,
                        source_line,
                        self.modal.motion == MotionMode::ClockwiseArc,
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
        self.advance(elapsed);
        let running = self.active.is_some() || !self.queued.is_empty();
        let mode = if self.held {
            "Hold:0"
        } else if running {
            "Run"
        } else {
            "Idle"
        };
        let feed = self
            .active
            .as_ref()
            .map_or(0.0, |active| active.motion.feed_mm_per_min);
        let work = subtract(self.position, work_offset);
        let line = self
            .active
            .as_ref()
            .and_then(|active| active.motion.source_line)
            .or(self.current_source_line);
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
            "Bf:15,128".to_owned(),
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
        if !running && !self.held {
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

    pub(crate) fn hold(&mut self) {
        if self.active.is_some() || !self.queued.is_empty() {
            self.advance_from_clock();
            self.held = true;
            self.manages_status = true;
        }
    }

    pub(crate) fn resume(&mut self) {
        if self.held {
            self.held = false;
            self.last_update = Instant::now();
            self.manages_status = self.active.is_some() || !self.queued.is_empty();
        }
    }

    pub(crate) fn stop(&mut self, position: [f64; 3]) {
        self.queued.clear();
        self.active = None;
        self.held = false;
        self.position = position;
        self.planned_position = position;
        self.current_source_line = None;
        self.manages_status = false;
        self.last_update = Instant::now();
        self.reset_modal();
    }

    pub(crate) fn sync_external_position(&mut self, position: [f64; 3]) {
        self.position = position;
        self.planned_position = position;
        self.queued.clear();
        self.active = None;
        self.held = false;
        self.manages_status = false;
        self.last_update = Instant::now();
    }

    pub(crate) fn advance_for_test(&mut self, elapsed: Duration) {
        self.advance(elapsed);
    }

    fn enqueue_linear(
        &mut self,
        start: [f64; 3],
        end: [f64; 3],
        source_line: Option<u32>,
        feed_mm_per_min: f64,
    ) {
        let distance = distance(start, end);
        if distance <= f64::EPSILON {
            return;
        }
        let seconds = match self.modal.feed_mode {
            FeedMode::UnitsPerMinute => distance / feed_mm_per_min.max(0.001) * 60.0,
            FeedMode::InverseTime => 60.0 / feed_mm_per_min.max(0.001),
        };
        self.enqueue_motion(PlannedMotion {
            end,
            duration: self.simulated_duration(seconds),
            source_line,
            feed_mm_per_min,
        });
    }

    fn enqueue_arc(
        &mut self,
        start: [f64; 3],
        end: [f64; 3],
        words: &[Word],
        source_line: Option<u32>,
        clockwise: bool,
    ) -> Result<(), u16> {
        let (u, v, w, center_u_letter, center_v_letter) = match self.modal.plane {
            Plane::Xy => (0, 1, 2, 'I', 'J'),
            Plane::Xz => (0, 2, 1, 'I', 'K'),
            Plane::Yz => (1, 2, 0, 'J', 'K'),
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
        let arc_length = radius * sweep.abs();
        let segments = ((arc_length / ARC_CHORD_MM).ceil() as usize)
            .max((sweep.abs() / (PI / 36.0)).ceil() as usize)
            .clamp(1, MAX_ARC_SEGMENTS);
        let feed = self.modal.feed.max(0.001);
        let mut previous = start;
        for index in 1..=segments {
            let t = index as f64 / segments as f64;
            let angle = start_angle + sweep * t;
            let mut point = end;
            point[u] = center[0] + radius * angle.cos();
            point[v] = center[1] + radius * angle.sin();
            point[w] = start[w] + (end[w] - start[w]) * t;
            if index == segments {
                point = end;
            }
            let segment_length = distance(previous, point);
            let seconds = match self.modal.feed_mode {
                FeedMode::UnitsPerMinute => segment_length / feed * 60.0,
                FeedMode::InverseTime => 60.0 / feed / segments as f64,
            };
            self.enqueue_motion(PlannedMotion {
                end: point,
                duration: self.simulated_duration(seconds),
                source_line,
                feed_mm_per_min: feed,
            });
            previous = point;
        }
        Ok(())
    }

    fn enqueue_motion(&mut self, motion: PlannedMotion) {
        self.queued.push_back(motion);
        self.manages_status = true;
        self.last_update = Instant::now();
    }

    fn advance_from_clock(&mut self) {
        let now = Instant::now();
        let elapsed = now.saturating_duration_since(self.last_update);
        self.last_update = now;
        self.advance(elapsed);
    }

    fn advance(&mut self, mut budget: Duration) {
        if self.held {
            return;
        }
        while !budget.is_zero() {
            if self.active.is_none() {
                let Some(motion) = self.queued.pop_front() else {
                    break;
                };
                self.current_source_line = motion.source_line;
                self.active = Some(ActiveMotion {
                    start: self.position,
                    motion,
                    elapsed: Duration::ZERO,
                });
            }
            let active = self.active.as_mut().expect("active motion initialized");
            let remaining = active.motion.duration.saturating_sub(active.elapsed);
            let consumed = budget.min(remaining);
            active.elapsed += consumed;
            budget -= consumed;
            let progress = if active.motion.duration.is_zero() {
                1.0
            } else {
                active.elapsed.as_secs_f64() / active.motion.duration.as_secs_f64()
            }
            .clamp(0.0, 1.0);
            self.position = interpolate(active.start, active.motion.end, progress);
            if active.elapsed >= active.motion.duration {
                self.position = active.motion.end;
                self.current_source_line = active.motion.source_line;
                self.active = None;
            } else {
                break;
            }
        }
    }

    fn simulated_duration(&self, physical_seconds: f64) -> Duration {
        Duration::from_secs_f64((physical_seconds / self.simulation_speed).max(0.0))
            .clamp(MIN_SIMULATED_MOTION, MAX_SIMULATED_MOTION)
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

fn format_position(name: &str, position: [f64; 3]) -> String {
    format!(
        "{name}:{:.3},{:.3},{:.3}",
        position[0], position[1], position[2]
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn finish(machine: &mut VirtualGrbl) {
        machine.advance_for_test(Duration::from_secs(60));
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
                false,
            )
            .unwrap();
        machine
            .execute_line("N2 G91 X2 Y-1", &offsets, &mut wcs, false)
            .unwrap();
        finish(&mut machine);

        assert_eq!(machine.position, [12.0, 4.0, -1.0]);
        assert_eq!(machine.current_source_line, Some(2));
    }

    #[test]
    fn interpolates_arcs_instead_of_jumping_across_the_chord() {
        let mut machine = VirtualGrbl::new([0.0; 3]);
        let offsets = [[0.0; 3]; 6];
        let mut wcs = 0;
        machine
            .execute_line("N1 G21 G90 G94 G1 X10 F300", &offsets, &mut wcs, false)
            .unwrap();
        finish(&mut machine);
        machine
            .execute_line("N2 G17 G3 X20 Y10 I0 J10 F300", &offsets, &mut wcs, false)
            .unwrap();
        machine.advance_for_test(Duration::from_millis(250));

        assert!(machine.position[0] > 10.0 && machine.position[0] < 20.0);
        assert!(machine.position[1] > 0.0 && machine.position[1] < 10.0);
        finish(&mut machine);
        assert_eq!(machine.position, [20.0, 10.0, 0.0]);
    }

    #[test]
    fn hold_freezes_and_resume_continues_the_same_motion() {
        let mut machine = VirtualGrbl::new([0.0; 3]);
        let offsets = [[0.0; 3]; 6];
        let mut wcs = 0;
        machine
            .execute_line("N7 G21 G90 G94 G1 X100 F100", &offsets, &mut wcs, false)
            .unwrap();
        machine.advance_for_test(Duration::from_millis(100));
        machine.hold();
        let held = machine.position;
        machine.advance_for_test(Duration::from_secs(10));
        assert_eq!(machine.position, held);

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
                false,
            )
            .unwrap();
        finish(&mut machine);

        assert_eq!(machine.position, [3.5, -2.0, 0.0]);
        assert_eq!(machine.current_source_line, Some(12));
    }

    #[test]
    fn inverse_time_linear_feed_is_a_block_duration() {
        let mut machine = VirtualGrbl::new([0.0; 3]);
        let offsets = [[0.0; 3]; 6];
        let mut wcs = 0;

        machine
            .execute_line("N3 G21 G90 G93 G1 X100 F1", &offsets, &mut wcs, false)
            .unwrap();
        machine.advance_for_test(Duration::from_millis(500));

        assert!(machine.position[0] > 0.0 && machine.position[0] < 100.0);
        finish(&mut machine);
        assert_eq!(machine.position, [100.0, 0.0, 0.0]);
    }
}
