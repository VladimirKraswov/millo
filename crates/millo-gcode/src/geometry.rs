use std::f64::consts::{PI, TAU};

use super::{ArcDistanceMode, POSITION_EPSILON_MM, Plane, ProgramPoint};

const ARC_MAX_ANGLE_RAD: f64 = PI / 36.0;
const ARC_MAX_CHORD_MM: f64 = 0.5;

pub(super) fn plane_offsets(
    plane: Plane,
    i: Option<f64>,
    j: Option<f64>,
    k: Option<f64>,
) -> (Option<f64>, Option<f64>, bool) {
    match plane {
        Plane::Xy => (i, j, k.is_some()),
        Plane::Xz => (k, i, j.is_some()),
        Plane::Yz => (j, k, i.is_some()),
    }
}

fn plane_components(point: ProgramPoint, plane: Plane) -> (f64, f64, f64) {
    match plane {
        Plane::Xy => (point.x, point.y, point.z),
        Plane::Xz => (point.z, point.x, point.y),
        Plane::Yz => (point.y, point.z, point.x),
    }
}

fn point_from_plane(u: f64, v: f64, linear: f64, plane: Plane) -> ProgramPoint {
    match plane {
        Plane::Xy => ProgramPoint {
            x: u,
            y: v,
            z: linear,
        },
        Plane::Xz => ProgramPoint {
            x: v,
            y: linear,
            z: u,
        },
        Plane::Yz => ProgramPoint {
            x: linear,
            y: u,
            z: v,
        },
    }
}

pub(super) struct ArcDefinition {
    pub(super) plane: Plane,
    pub(super) offset_u: Option<f64>,
    pub(super) offset_v: Option<f64>,
    pub(super) radius: Option<f64>,
    pub(super) clockwise: bool,
    pub(super) distance_mode: ArcDistanceMode,
}

#[derive(Debug, PartialEq, Eq)]
pub(super) enum ArcError {
    InvalidDefinition,
    PreviewLimit,
}

pub(super) fn sample_arc(
    start: ProgramPoint,
    end: ProgramPoint,
    definition: ArcDefinition,
    point_budget: usize,
) -> Result<Vec<ProgramPoint>, ArcError> {
    let (start_u, start_v, start_linear) = plane_components(start, definition.plane);
    let (end_u, end_v, end_linear) = plane_components(end, definition.plane);
    let (center_u, center_v) = if definition.offset_u.is_some() || definition.offset_v.is_some() {
        match definition.distance_mode {
            ArcDistanceMode::Incremental => (
                start_u + definition.offset_u.unwrap_or(0.0),
                start_v + definition.offset_v.unwrap_or(0.0),
            ),
            ArcDistanceMode::Absolute => (
                definition.offset_u.ok_or(ArcError::InvalidDefinition)?,
                definition.offset_v.ok_or(ArcError::InvalidDefinition)?,
            ),
        }
    } else {
        center_from_radius(
            start_u,
            start_v,
            end_u,
            end_v,
            definition.radius.ok_or(ArcError::InvalidDefinition)?,
            definition.clockwise,
        )
        .ok_or(ArcError::InvalidDefinition)?
    };
    let arc_radius = (start_u - center_u).hypot(start_v - center_v);
    if !arc_radius.is_finite() || arc_radius <= POSITION_EPSILON_MM {
        return Err(ArcError::InvalidDefinition);
    }
    let end_radius = (end_u - center_u).hypot(end_v - center_v);
    let radius_error = (arc_radius - end_radius).abs();
    // GRBL 1.1: 5 um absolute tolerance, then both 0.5 mm and 0.1% limits.
    if !end_radius.is_finite()
        || (radius_error > 0.005 && (radius_error > 0.5 || radius_error > arc_radius * 0.001))
    {
        return Err(ArcError::InvalidDefinition);
    }

    let start_angle = (start_v - center_v).atan2(start_u - center_u);
    let end_angle = (end_v - center_v).atan2(end_u - center_u);
    let full_circle = (start_u - end_u).abs() <= POSITION_EPSILON_MM
        && (start_v - end_v).abs() <= POSITION_EPSILON_MM;
    let sweep = directed_sweep(start_angle, end_angle, definition.clockwise, full_circle);
    if !sweep.is_finite() || sweep <= POSITION_EPSILON_MM {
        return Err(ArcError::InvalidDefinition);
    }
    let angle_steps = (sweep / ARC_MAX_ANGLE_RAD).ceil();
    let chord_steps = (arc_radius * sweep / ARC_MAX_CHORD_MM).ceil();
    let required_steps = angle_steps.max(chord_steps).max(2.0);
    if !required_steps.is_finite() || required_steps + 1.0 > point_budget as f64 {
        return Err(ArcError::PreviewLimit);
    }
    let steps = required_steps as usize;
    // Include cardinal extrema so bounds are not dependent on tessellation phase.
    let mut extrema = Vec::with_capacity(4);
    for quadrant in 0..4 {
        let angle = quadrant as f64 * PI / 2.0;
        let progress = directed_sweep(start_angle, angle, definition.clockwise, false) / sweep;
        if progress > 0.0 && progress < 1.0 {
            extrema.push(progress);
        }
    }
    if steps + 1 + extrema.len() > point_budget {
        return Err(ArcError::PreviewLimit);
    }
    let mut samples = (0..=steps)
        .map(|step| step as f64 / steps as f64)
        .collect::<Vec<_>>();
    samples.extend(extrema);
    samples.sort_unstable_by(f64::total_cmp);
    samples.dedup_by(|left, right| (*left - *right).abs() < f64::EPSILON);
    let mut points = Vec::with_capacity(samples.len());
    for progress in samples {
        let angle = if definition.clockwise {
            start_angle - sweep * progress
        } else {
            start_angle + sweep * progress
        };
        points.push(point_from_plane(
            center_u + arc_radius * angle.cos(),
            center_v + arc_radius * angle.sin(),
            start_linear + (end_linear - start_linear) * progress,
            definition.plane,
        ));
    }
    if let Some(first) = points.first_mut() {
        *first = start;
    }
    if let Some(last) = points.last_mut() {
        *last = end;
    }
    Ok(points)
}

fn center_from_radius(
    start_u: f64,
    start_v: f64,
    end_u: f64,
    end_v: f64,
    signed_radius: f64,
    clockwise: bool,
) -> Option<(f64, f64)> {
    let dx = end_u - start_u;
    let dy = end_v - start_v;
    let chord = dx.hypot(dy);
    let radius = signed_radius.abs();
    if chord <= POSITION_EPSILON_MM || radius + POSITION_EPSILON_MM < chord / 2.0 {
        return None;
    }
    let midpoint = ((start_u + end_u) / 2.0, (start_v + end_v) / 2.0);
    let height = (radius * radius - chord * chord / 4.0).max(0.0).sqrt();
    let perpendicular = (-dy / chord, dx / chord);
    let candidates = [
        (
            midpoint.0 + perpendicular.0 * height,
            midpoint.1 + perpendicular.1 * height,
        ),
        (
            midpoint.0 - perpendicular.0 * height,
            midpoint.1 - perpendicular.1 * height,
        ),
    ];
    let wants_major = signed_radius < 0.0;
    candidates.into_iter().min_by(|left, right| {
        let score = |center: &(f64, f64)| {
            let start_angle = (start_v - center.1).atan2(start_u - center.0);
            let end_angle = (end_v - center.1).atan2(end_u - center.0);
            let sweep = directed_sweep(start_angle, end_angle, clockwise, false);
            let is_major = sweep > PI + 1e-6;
            if is_major == wants_major { 0 } else { 1 }
        };
        score(left).cmp(&score(right))
    })
}

fn directed_sweep(start_angle: f64, end_angle: f64, clockwise: bool, full: bool) -> f64 {
    if full {
        TAU
    } else if clockwise {
        (start_angle - end_angle).rem_euclid(TAU)
    } else {
        (end_angle - start_angle).rem_euclid(TAU)
    }
}

pub(super) fn polyline_distance(points: &[ProgramPoint]) -> f64 {
    points
        .windows(2)
        .map(|pair| {
            let dx = pair[1].x - pair[0].x;
            let dy = pair[1].y - pair[0].y;
            let dz = pair[1].z - pair[0].z;
            dx.hypot(dy).hypot(dz)
        })
        .sum()
}
