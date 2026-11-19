use codemorph_core::model::ModelRole;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelManifest {
    pub model_id: String,
    pub version: String,
    pub role: ModelRole,
    pub source_language: String,
    pub target_language: String,
    pub supported_features: Vec<String>,
    pub architecture: String,
    pub parameter_count: u64,
    pub tokenizer: String,
    pub context_length: usize,
    pub backend: String,
    pub artifact_format: String,
    pub protocol_version: String,
    #[serde(default)]
    pub schema_version: Option<String>,
    #[serde(default)]
    pub model_version: Option<String>,
    #[serde(default)]
    pub dataset_version: Option<String>,
    #[serde(default)]
    pub training_config_version: Option<String>,
    #[serde(default)]
    pub language_pair: Option<String>,
    pub hashes: std::collections::BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManifestValidation {
    pub manifest_path: String,
    pub valid: bool,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
    pub artifacts: Vec<String>,
}

impl ModelManifest {
    pub fn load(path: &Path) -> Result<Self, Box<dyn std::error::Error>> {
        let text = fs::read_to_string(path)?;
        Ok(serde_json::from_str(&text)?)
    }
}

pub fn sha256_file(path: &Path) -> Result<String, std::io::Error> {
    let bytes = fs::read(path)?;
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    Ok(hex(hasher.finalize().as_slice()))
}

fn hex(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push_str(&format!("{:02x}", b));
    }
    s
}

pub struct ModelRegistry {
    pub root: PathBuf,
}
impl ModelRegistry {
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    pub fn discover(&self) -> Vec<PathBuf> {
        let mut manifests = Vec::new();
        if let Ok(entries) = fs::read_dir(&self.root) {
            for entry in entries.flatten() {
                let dir = entry.path();
                if dir.is_dir() {
                    let manifest = dir.join("manifest.json");
                    if manifest.exists() {
                        manifests.push(manifest);
                    }
                }
            }
        }
        manifests.sort();
        manifests
    }

    pub fn validate_manifest(&self, path: &Path) -> ManifestValidation {
        let mut errors = Vec::new();
        let mut warnings = Vec::new();
        let mut artifacts = Vec::new();

        let manifest = match ModelManifest::load(path) {
            Ok(m) => m,
            Err(e) => {
                return ManifestValidation {
                    manifest_path: path.display().to_string(),
                    valid: false,
                    errors: vec![format!("failed to parse manifest: {}", e)],
                    warnings,
                    artifacts,
                }
            }
        };

        let base = path.parent().unwrap_or_else(|| Path::new("."));
        for (name, expected) in &manifest.hashes {
            let file = base.join(name);
            if !file.exists() {
                errors.push(format!("missing artifact {}", name));
                continue;
            }
            artifacts.push(file.display().to_string());
            match sha256_file(&file) {
                Ok(actual) if actual == *expected => {}
                Ok(actual) => errors.push(format!(
                    "hash mismatch for {}: expected {}, actual {}",
                    name, expected, actual
                )),
                Err(e) => errors.push(format!("could not hash {}: {}", name, e)),
            }
        }
        if manifest.hashes.is_empty() {
            warnings.push("manifest declares no artifacts".to_string());
        }
        if !base.join(&manifest.tokenizer).exists() && !manifest.tokenizer.is_empty() {
            // tokenizer can be a directory/file; warn rather than reject.
            warnings.push(format!("tokenizer path not found: {}", manifest.tokenizer));
        }

        ManifestValidation {
            manifest_path: path.display().to_string(),
            valid: errors.is_empty(),
            errors,
            warnings,
            artifacts,
        }
    }

    pub fn validate_all(&self) -> Vec<ManifestValidation> {
        self.discover()
            .into_iter()
            .map(|p| self.validate_manifest(&p))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn invalid_manifest_reports_error() {
        let dir = TempDir::new().unwrap();
        let mdir = dir.path().join("translator");
        fs::create_dir_all(&mdir).unwrap();
        fs::write(mdir.join("manifest.json"), "{not json}").unwrap();
        let reg = ModelRegistry::new(dir.path().to_path_buf());
        let results = reg.validate_all();
        assert_eq!(results.len(), 1);
        assert!(!results[0].valid);
    }

    #[test]
    fn valid_manifest_with_hash() {
        let dir = TempDir::new().unwrap();
        let mdir = dir.path().join("translator");
        fs::create_dir_all(&mdir).unwrap();
        fs::write(mdir.join("weights.bin"), b"abc").unwrap();
        let hash = sha256_file(&mdir.join("weights.bin")).unwrap();
        let manifest = serde_json::json!({
            "model_id": "codemorph/test/translator",
            "version": "1",
            "role": "Translator",
            "source_language": "cpp",
            "target_language": "rust",
            "supported_features": ["arithmetic"],
            "architecture": "transformer",
            "parameter_count": 1000,
            "tokenizer": "tokenizer",
            "context_length": 512,
            "backend": "test",
            "artifact_format": "safetensors",
            "protocol_version": "1",
            "hashes": {"weights.bin": hash}
        });
        fs::write(mdir.join("manifest.json"), manifest.to_string()).unwrap();
        let reg = ModelRegistry::new(dir.path().to_path_buf());
        let results = reg.validate_all();
        assert!(results[0].valid, "{:?}", results[0].errors);
    }
}
