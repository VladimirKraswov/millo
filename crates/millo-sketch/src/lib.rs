mod constraints;
mod geometry;
mod model;
mod planner;
mod postprocessor;

pub use constraints::resolve_sketch;
use millo_tooling::CuttingTool;
pub use model::*;
use thiserror::Error;

const MAX_SHAPES: usize = 200;
const MAX_POINTS: usize = 100_000;
const MAX_PASSES: usize = 200;

#[derive(Debug, Error, PartialEq)]
#[error("{0}")]
pub struct SketchError(pub String);

pub fn project_file_name(name: &str) -> String {
    let gcode_name = postprocessor::filename(name);
    format!(
        "{}.millo-sketch.json",
        gcode_name.strip_suffix(".nc").unwrap_or(&gcode_name)
    )
}

pub fn generate_sketch_job(
    request: SketchJobRequest,
    tools: &[CuttingTool],
) -> Result<GeneratedSketchJob, SketchError> {
    let request = resolve_sketch(request)?;
    let plan = planner::plan(&request, tools)?;
    postprocessor::generate(&request, &plan)
}

fn require(condition: bool, message: impl Into<String>) -> Result<(), SketchError> {
    if condition {
        Ok(())
    } else {
        Err(SketchError(message.into()))
    }
}

fn range(value: f64, min: f64, max: f64, name: &str) -> Result<(), SketchError> {
    require(
        value.is_finite() && value >= min && value <= max,
        format!("{name}: допустимо от {min} до {max}"),
    )
}
