use codemorph_core::pipeline::{PipelineState, StageResult};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnitState {
    pub id: String,
    pub stage: PipelineState,
    pub attempts: u32,
    pub accepted: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransitionLog {
    pub from: PipelineState,
    pub to: PipelineState,
    pub unit: Option<String>,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobBudgets {
    pub max_candidates: u32,
    pub max_repairs: u32,
    pub max_tokens: u64,
    pub timeout_seconds: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrationJob {
    pub id: String,
    pub input_root: String,
    pub output_root: String,
    pub state: PipelineState,
    pub units: Vec<UnitState>,
    pub current_unit: Option<usize>,
    pub budgets: JobBudgets,
    pub history: Vec<TransitionLog>,
    pub stage_results: Vec<StageResult>,
}

impl MigrationJob {
    pub fn new(
        id: impl Into<String>,
        input_root: impl Into<String>,
        output_root: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            input_root: input_root.into(),
            output_root: output_root.into(),
            state: PipelineState::AnalyzeProject,
            units: Vec::new(),
            current_unit: None,
            budgets: JobBudgets {
                max_candidates: 5,
                max_repairs: 3,
                max_tokens: 100_000,
                timeout_seconds: 3600,
            },
            history: Vec::new(),
            stage_results: Vec::new(),
        }
    }

    pub fn transition(&mut self, to: PipelineState, message: impl Into<String>) {
        let entry = TransitionLog {
            from: self.state,
            to,
            unit: self.current_unit.map(|i| self.units[i].id.clone()),
            message: message.into(),
        };
        self.state = to;
        self.history.push(entry);
    }

    pub fn advance_deterministic(&mut self) -> PipelineState {
        let next = match self.state {
            PipelineState::AnalyzeProject => PipelineState::BuildGraph,
            PipelineState::BuildGraph => PipelineState::SelectUnit,
            PipelineState::SelectUnit => {
                if self.units.is_empty() {
                    PipelineState::Complete
                } else {
                    self.current_unit = Some(0);
                    PipelineState::Translate
                }
            }
            PipelineState::Translate => PipelineState::Compile,
            PipelineState::Compile => PipelineState::Test,
            PipelineState::Test => PipelineState::Accept,
            PipelineState::Accept => PipelineState::SelectUnit,
            PipelineState::Retry => PipelineState::Translate,
            PipelineState::Escalate => PipelineState::SelectUnit,
            other => other,
        };
        if next != self.state {
            self.transition(next, "deterministic advance");
        }
        self.state
    }

    pub fn save(&self, path: &Path) -> Result<(), std::io::Error> {
        let tmp = path.with_extension("tmp");
        let data = serde_json::to_string_pretty(self).map_err(std::io::Error::other)?;
        fs::write(&tmp, data)?;
        fs::rename(tmp, path)?;
        Ok(())
    }

    pub fn load(path: &Path) -> Result<Self, std::io::Error> {
        let data = fs::read_to_string(path)?;
        serde_json::from_str(&data).map_err(std::io::Error::other)
    }
}

pub fn state_path(dir: &Path) -> PathBuf {
    dir.join(".codemorph").join("job.json")
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    #[test]
    fn advances_to_complete_for_no_units() {
        let mut job = MigrationJob::new("j", "in", "out");
        while job.state != PipelineState::Complete {
            let s = job.advance_deterministic();
            if s == PipelineState::Complete {
                break;
            }
        }
        assert_eq!(job.state, PipelineState::Complete);
    }
    #[test]
    fn persists_and_loads() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("job.json");
        let job = MigrationJob::new("j", "in", "out");
        job.save(&path).unwrap();
        let loaded = MigrationJob::load(&path).unwrap();
        assert_eq!(loaded.id, "j");
    }
}
