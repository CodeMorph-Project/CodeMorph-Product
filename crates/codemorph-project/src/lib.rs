use codemorph_ir::{ProjectFile, ProjectIr};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::Path;

pub fn detect_language(path: &Path) -> Option<&'static str> {
    let ext = path.extension()?.to_str()?.to_lowercase();
    match ext.as_str() {
        "c" => Some("c"),
        "cpp" | "cc" | "cxx" | "hpp" | "hh" | "h" => Some("cpp"),
        "rs" => Some("rust"),
        "java" => Some("java"),
        "kt" => Some("kotlin"),
        "py" => Some("python"),
        "js" | "ts" => Some("javascript"),
        "cu" => Some("cuda"),
        _ => None,
    }
}

pub fn sha256_file(path: &Path) -> Result<String, std::io::Error> {
    let bytes = fs::read(path)?;
    let mut h = Sha256::new();
    h.update(bytes);
    Ok(hex(h.finalize().as_slice()))
}

fn hex(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push_str(&format!("{:02x}", b));
    }
    s
}

/// Analyze a source file or walk a project directory without following nested symlinks.
pub fn analyze(root: &Path) -> ProjectIr {
    let mut ir = ProjectIr::new(root.display().to_string());
    if root.is_file() {
        record_file(
            root.parent().unwrap_or_else(|| Path::new("")),
            root,
            &mut ir,
        );
    } else {
        walk(root, root, &mut ir);
    }
    ir.files.sort_by(|left, right| left.path.cmp(&right.path));
    ir
}

fn walk(root: &Path, dir: &Path, ir: &mut ProjectIr) {
    let Ok(read) = fs::read_dir(dir) else {
        return;
    };
    for entry in read.flatten() {
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        let path = entry.path();
        if kind.is_dir() {
            walk(root, &path, ir);
        } else if kind.is_file() {
            record_file(root, &path, ir);
        }
    }
}

fn record_file(root: &Path, path: &Path, ir: &mut ProjectIr) {
    let Some(language) = detect_language(path) else {
        return;
    };
    let relative = path.strip_prefix(root).unwrap_or(path);
    ir.files.push(ProjectFile {
        path: relative.display().to_string(),
        language: language.to_string(),
        hash: sha256_file(path).unwrap_or_default(),
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;
    #[test]
    fn analyze_counts_files() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("main.cpp"), "int main(){}").unwrap();
        fs::write(dir.path().join("lib.rs"), "fn main(){}").unwrap();
        let ir = analyze(dir.path());
        assert_eq!(ir.files.len(), 2);
    }

    #[test]
    fn analyzes_single_file() {
        let dir = TempDir::new().unwrap();
        let input = dir.path().join("main.cpp");
        fs::write(&input, "int main(){}").unwrap();
        let ir = analyze(&input);
        assert_eq!(ir.files.len(), 1);
        assert_eq!(ir.files[0].path, "main.cpp");
        assert_eq!(ir.files[0].hash, sha256_file(&input).unwrap());
    }
}
