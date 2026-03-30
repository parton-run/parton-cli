//! Execution stage definitions.

use serde::{Deserialize, Serialize};

/// Identifies which stage of the pipeline a provider is being used for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum StageKind {
    /// Fast context selection: pick relevant files for the planner.
    Context,
    /// High-level planning: decompose work, identify dependencies.
    Planning,
    /// Code generation and editing.
    Execution,
    /// Review and validate changes against acceptance criteria.
    Judge,
}

impl std::fmt::Display for StageKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Context => write!(f, "context"),
            Self::Planning => write!(f, "planning"),
            Self::Execution => write!(f, "execution"),
            Self::Judge => write!(f, "judge"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_formats_lowercase() {
        assert_eq!(StageKind::Context.to_string(), "context");
        assert_eq!(StageKind::Planning.to_string(), "planning");
        assert_eq!(StageKind::Execution.to_string(), "execution");
        assert_eq!(StageKind::Judge.to_string(), "judge");
    }

    #[test]
    fn serde_roundtrip() {
        let stage = StageKind::Execution;
        let json = serde_json::to_string(&stage).unwrap();
        assert_eq!(json, r#""execution""#);
        let parsed: StageKind = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, stage);
    }
}
