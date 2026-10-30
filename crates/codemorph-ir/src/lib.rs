//! Generic project and source representation.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceSpan {
    pub file: String,
    pub start_line: u32,
    pub start_column: u32,
    pub end_line: u32,
    pub end_column: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectFile {
    pub path: String,
    pub language: String,
    pub hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Symbol {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub span: Option<SourceSpan>,
    pub metadata: BTreeMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DependencyEdge {
    pub from: String,
    pub to: String,
    pub kind: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectIr {
    pub version: String,
    pub root: String,
    pub files: Vec<ProjectFile>,
    pub symbols: Vec<Symbol>,
    pub dependencies: Vec<DependencyEdge>,
    pub extensions: BTreeMap<String, serde_json::Value>,
}

impl ProjectIr {
    pub fn new(root: impl Into<String>) -> Self {
        Self {
            version: "0.1".to_string(),
            root: root.into(),
            files: Vec::new(),
            symbols: Vec::new(),
            dependencies: Vec::new(),
            extensions: BTreeMap::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn project_ir_is_serializable() {
        let ir = ProjectIr::new("/tmp/p");
        let json = serde_json::to_string(&ir).unwrap();
        let back: ProjectIr = serde_json::from_str(&json).unwrap();
        assert_eq!(back.version, "0.1");
    }
}
