use codemorph_core::verification::{VerificationResult, VerificationStage};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BinaryOutput {
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
    pub timed_out: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DifferentialOutcome {
    pub input: String,
    pub cpp: BinaryOutput,
    pub rust: BinaryOutput,
    pub match_stdout: bool,
    pub match_exit: bool,
    pub notes: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DifferentialSummary {
    pub cases: usize,
    pub passed: usize,
    pub failed: usize,
    pub mismatches: Vec<DifferentialOutcome>,
}

impl Default for DifferentialSummary {
    fn default() -> Self {
        Self::new()
    }
}

impl DifferentialSummary {
    pub fn new() -> Self {
        Self {
            cases: 0,
            passed: 0,
            failed: 0,
            mismatches: Vec::new(),
        }
    }
    pub fn record(&mut self, outcome: DifferentialOutcome) {
        self.cases += 1;
        if outcome.match_stdout && outcome.match_exit {
            self.passed += 1;
        } else {
            self.failed += 1;
            self.mismatches.push(outcome);
        }
    }
}

/// A tiny reproducible PRNG for test input generation.
pub struct SeededRng {
    state: u64,
}
impl SeededRng {
    pub fn new(seed: u64) -> Self {
        Self {
            state: seed.wrapping_add(0x9E3779B97F4A7C15),
        }
    }
    pub fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E3779B97F4A7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111B9);
        z ^ (z >> 31)
    }
    pub fn next_i32(&mut self, min: i32, max: i32) -> i32 {
        if min >= max {
            return min;
        }
        let span = (max as i64 - min as i64 + 1) as u64;
        let v = (self.next_u64() % span) as i64;
        (min as i64 + v) as i32
    }
}

pub fn build_verification_result(summary: &DifferentialSummary) -> VerificationResult {
    let mut vr = VerificationResult::new();
    vr.cpp_compiles = true;
    vr.rust_compiles = true;
    vr.differential_passed = summary.failed == 0 && summary.cases > 0;
    vr.tests_passed = vr.differential_passed;
    vr.confidence = if vr.differential_passed {
        codemorph_core::verification::Confidence::Medium
    } else {
        codemorph_core::verification::Confidence::None
    };
    if summary.failed > 0 {
        vr.add_rejection(format!("{} differential mismatches", summary.failed));
    }
    vr.stages.push(VerificationStage {
        name: "differential".to_string(),
        success: vr.differential_passed,
        details: format!("{}/{} cases passed", summary.passed, summary.cases),
    });
    vr
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rng_is_deterministic() {
        let mut a = SeededRng::new(1);
        let mut b = SeededRng::new(1);
        assert_eq!(a.next_u64(), b.next_u64());
    }
    #[test]
    fn differential_summary_counts() {
        let mut s = DifferentialSummary::new();
        s.record(DifferentialOutcome {
            input: "x".to_string(),
            cpp: BinaryOutput {
                exit_code: Some(0),
                stdout: "1\n".to_string(),
                stderr: String::new(),
                timed_out: false,
            },
            rust: BinaryOutput {
                exit_code: Some(0),
                stdout: "1\n".to_string(),
                stderr: String::new(),
                timed_out: false,
            },
            match_stdout: true,
            match_exit: true,
            notes: String::new(),
        });
        assert_eq!(s.passed, 1);
        assert!(build_verification_result(&s).differential_passed);
    }
}
