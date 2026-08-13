use millo_domain::{ControllerSnapshot, JogAxis, WorkAxis};
use millo_script::{ScriptAxis, ScriptCapability, ScriptGeneratedJob, ScriptNoticeTone};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScriptPluginSourceRequest {
    pub package_json: String,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScriptPluginEnableRequest {
    pub plugin_id: String,
    pub digest: String,
    pub enabled: bool,
    pub granted_capabilities: Vec<ScriptCapability>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScriptPluginDeleteRequest {
    pub plugin_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScriptPluginExportRequest {
    pub plugin_id: String,
    pub digest: String,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScriptPluginExecutionRequest {
    pub plugin_id: String,
    pub digest: String,
    pub command_id: String,
    pub input: Value,
    #[serde(default)]
    pub operator_confirmed: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum ScriptPluginExecutionOutcome {
    Job {
        job: ScriptGeneratedJob,
    },
    Machine {
        action: String,
        message: String,
        snapshot: ControllerSnapshot,
    },
    Notice {
        title: String,
        message: String,
        tone: ScriptNoticeTone,
    },
}

pub(super) fn ensure_script_motion_confirmed(confirmed: bool) -> Result<(), String> {
    if confirmed {
        Ok(())
    } else {
        Err("operator confirmation is required for a plugin machine action".to_owned())
    }
}

pub(super) fn jog_axis(axis: ScriptAxis) -> JogAxis {
    match axis {
        ScriptAxis::X => JogAxis::X,
        ScriptAxis::Y => JogAxis::Y,
        ScriptAxis::Z => JogAxis::Z,
    }
}

pub(super) fn work_axis(axis: ScriptAxis) -> WorkAxis {
    match axis {
        ScriptAxis::X => WorkAxis::X,
        ScriptAxis::Y => WorkAxis::Y,
        ScriptAxis::Z => WorkAxis::Z,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plugin_motion_requires_an_explicit_operator_confirmation() {
        assert!(ensure_script_motion_confirmed(true).is_ok());
        assert!(ensure_script_motion_confirmed(false).is_err());
    }

    #[test]
    fn script_axes_map_to_their_typed_machine_axes() {
        for (source, jog, work) in [
            (ScriptAxis::X, JogAxis::X, WorkAxis::X),
            (ScriptAxis::Y, JogAxis::Y, WorkAxis::Y),
            (ScriptAxis::Z, JogAxis::Z, WorkAxis::Z),
        ] {
            assert_eq!(jog_axis(source), jog);
            assert_eq!(work_axis(source), work);
        }
    }
}
