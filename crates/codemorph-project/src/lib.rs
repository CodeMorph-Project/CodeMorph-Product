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

pub fn analyze(root: &Path) -> ProjectIr {
    let mut ir = ProjectIr::new(root.display().to_string());
    walk(root, root, &mut ir);
    ir
}

fn walk(root: &Path, dir: &Path, ir: &mut ProjectIr) {
    let Ok(read) = fs::read_dir(dir) else {
        return;
    };
    for entry in read.flatten() {
        let path = entry.path();
        if path.is_dir() {
            walk(root, &path, ir);
        } else if let Some(lang) = detect_language(&path) {
            let rel = path
                .strip_prefix(root)
                .unwrap_or(&path)
                .display()
                .to_string();
            let hash = sha256_file(&path).unwrap_or_default();
            ir.files.push(ProjectFile {
                path: rel,
                language: lang.to_string(),
                hash,
            });
        }
    }
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
}
