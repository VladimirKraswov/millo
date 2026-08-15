use crate::{PcbCopperAnalysis, PcbLayerRole, geometry::BoardGeometry};

const RELEVANT_GAP_LIMIT_MM: f64 = 2.0;
const MAX_SEGMENT_COMPARISONS: usize = 5_000_000;

#[derive(Clone, Copy)]
struct Segment {
    path_index: usize,
    start: (f64, f64),
    end: (f64, f64),
    min_x: f64,
    max_x: f64,
    min_y: f64,
    max_y: f64,
}

pub(crate) fn analyze_copper(board: &BoardGeometry) -> PcbCopperAnalysis {
    let paths = board
        .layers
        .iter()
        .filter(|layer| layer.role == PcbLayerRole::Copper)
        .flat_map(|layer| layer.paths.iter())
        .collect::<Vec<_>>();
    // Clipper emits holes with the opposite winding. They describe empty space
    // inside one copper island, not clearance between two independent islands.
    let exterior_paths = paths
        .iter()
        .filter(|path| path.signed_area() > 0.0)
        .collect::<Vec<_>>();
    let mut segments = exterior_paths
        .iter()
        .enumerate()
        .flat_map(|(path_index, path)| {
            let points = path
                .iter()
                .map(|point| (point.x(), point.y()))
                .collect::<Vec<_>>();
            (0..points.len()).map(move |index| {
                let start = points[index];
                let end = points[(index + 1) % points.len()];
                Segment {
                    path_index,
                    start,
                    end,
                    min_x: start.0.min(end.0),
                    max_x: start.0.max(end.0),
                    min_y: start.1.min(end.1),
                    max_y: start.1.max(end.1),
                }
            })
        })
        .collect::<Vec<_>>();
    segments.sort_by(|left, right| left.min_x.total_cmp(&right.min_x));

    let mut minimum = RELEVANT_GAP_LIMIT_MM;
    let mut comparisons = 0usize;
    'outer: for (index, left) in segments.iter().enumerate() {
        for right in segments.iter().skip(index + 1) {
            if right.min_x - left.max_x > minimum {
                break;
            }
            if left.path_index == right.path_index
                || interval_gap(left.min_y, left.max_y, right.min_y, right.max_y) > minimum
            {
                continue;
            }
            comparisons += 1;
            if comparisons > MAX_SEGMENT_COMPARISONS {
                break 'outer;
            }
            minimum = minimum.min(segment_distance(
                left.start,
                left.end,
                right.start,
                right.end,
            ));
            if minimum <= 0.001 {
                break 'outer;
            }
        }
    }

    PcbCopperAnalysis {
        contour_count: paths.len(),
        minimum_isolation_gap_mm: (minimum < RELEVANT_GAP_LIMIT_MM).then_some(minimum),
    }
}

fn interval_gap(left_min: f64, left_max: f64, right_min: f64, right_max: f64) -> f64 {
    if left_max < right_min {
        right_min - left_max
    } else if right_max < left_min {
        left_min - right_max
    } else {
        0.0
    }
}

fn segment_distance(a: (f64, f64), b: (f64, f64), c: (f64, f64), d: (f64, f64)) -> f64 {
    if segments_intersect(a, b, c, d) {
        return 0.0;
    }
    point_segment_distance(a, c, d)
        .min(point_segment_distance(b, c, d))
        .min(point_segment_distance(c, a, b))
        .min(point_segment_distance(d, a, b))
}

fn point_segment_distance(point: (f64, f64), start: (f64, f64), end: (f64, f64)) -> f64 {
    let delta = (end.0 - start.0, end.1 - start.1);
    let length_squared = delta.0 * delta.0 + delta.1 * delta.1;
    if length_squared <= f64::EPSILON {
        return (point.0 - start.0).hypot(point.1 - start.1);
    }
    let projection = (((point.0 - start.0) * delta.0 + (point.1 - start.1) * delta.1)
        / length_squared)
        .clamp(0.0, 1.0);
    let closest = (
        start.0 + projection * delta.0,
        start.1 + projection * delta.1,
    );
    (point.0 - closest.0).hypot(point.1 - closest.1)
}

fn segments_intersect(a: (f64, f64), b: (f64, f64), c: (f64, f64), d: (f64, f64)) -> bool {
    let cross = |origin: (f64, f64), left: (f64, f64), right: (f64, f64)| {
        (left.0 - origin.0) * (right.1 - origin.1) - (left.1 - origin.1) * (right.0 - origin.0)
    };
    let ab_c = cross(a, b, c);
    let ab_d = cross(a, b, d);
    let cd_a = cross(c, d, a);
    let cd_b = cross(c, d, b);
    (ab_c.signum() != ab_d.signum() && cd_a.signum() != cd_b.signum())
        || ab_c.abs() <= 1e-9 && point_segment_distance(c, a, b) <= 1e-9
        || ab_d.abs() <= 1e-9 && point_segment_distance(d, a, b) <= 1e-9
        || cd_a.abs() <= 1e-9 && point_segment_distance(a, c, d) <= 1e-9
        || cd_b.abs() <= 1e-9 && point_segment_distance(b, c, d) <= 1e-9
}

#[cfg(test)]
mod tests {
    use clipper2::Paths;

    use super::*;
    use crate::geometry::{LayerGeometry, rectangle};

    #[test]
    fn reports_the_gap_between_separate_copper_contours() {
        let board = BoardGeometry {
            layers: vec![LayerGeometry {
                source_name: "fixture.gbr".to_owned(),
                role: PcbLayerRole::Copper,
                paths: Paths::new(vec![
                    rectangle(
                        crate::PcbPoint {
                            x_mm: 0.5,
                            y_mm: 0.5,
                        },
                        1.0,
                        1.0,
                    ),
                    rectangle(
                        crate::PcbPoint {
                            x_mm: 1.7,
                            y_mm: 0.5,
                        },
                        1.0,
                        1.0,
                    ),
                ]),
            }],
            ..BoardGeometry::default()
        };

        let analysis = analyze_copper(&board);
        assert_eq!(analysis.contour_count, 2);
        assert!((analysis.minimum_isolation_gap_mm.unwrap() - 0.2).abs() < 0.001);
    }

    #[test]
    fn ignores_hole_boundaries_inside_a_copper_island() {
        let outer = rectangle(
            crate::PcbPoint {
                x_mm: 2.0,
                y_mm: 2.0,
            },
            4.0,
            4.0,
        );
        let mut hole_points = rectangle(
            crate::PcbPoint {
                x_mm: 2.0,
                y_mm: 2.0,
            },
            3.8,
            3.8,
        )
        .iter()
        .map(|point| (point.x(), point.y()))
        .collect::<Vec<_>>();
        hole_points.reverse();
        let board = BoardGeometry {
            layers: vec![LayerGeometry {
                source_name: "fixture.gbr".to_owned(),
                role: PcbLayerRole::Copper,
                paths: Paths::new(vec![outer, hole_points.into()]),
            }],
            ..BoardGeometry::default()
        };

        let analysis = analyze_copper(&board);
        assert_eq!(analysis.contour_count, 2);
        assert_eq!(analysis.minimum_isolation_gap_mm, None);
    }
}
