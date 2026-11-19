use codemorph_core::sandbox::{ExecutionLimits, ExecutionRequest, ExecutionResult};
use std::fs;
use std::io::{self, Read, Seek, Write};
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};
use wait_timeout::ChildExt;

const DEFAULT_IMAGE: &str = "rust:1-bookworm";

/// Compile a standalone source file in an isolated container and copy back its binary.
pub fn compile(
    compiler: &str,
    flags: &[String],
    source: &Path,
    output: &Path,
    limits: &ExecutionLimits,
) -> io::Result<ExecutionResult> {
    let scratch = tempfile::tempdir()?;
    let binary_dir = scratch.path().join("output");
    fs::create_dir(&binary_dir)?;
    // The container uses a fixed unprivileged UID. The parent is private to the host user.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&binary_dir, fs::Permissions::from_mode(0o777))?;
    }
    let output_mount = mount(&binary_dir, "/output", false)?;
    let extension = source
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or("");
    // rustc uses the basename to derive a valid crate name.
    let container_source = format!("/source/input.{extension}");
    let input = mount(source, &container_source, true)?;
    let args: Vec<_> = flags.iter().map(String::as_str).collect();
    let mut command = vec![compiler];
    command.extend(args);
    command.extend([&container_source, "-o", "/output/program"]);
    let result = run_container(&[input, output_mount], &command, None, limits)?;
    if result.exit_code == Some(0) {
        let candidate = binary_dir.join("program");
        let metadata = fs::symlink_metadata(&candidate)?;
        if !metadata.is_file() || metadata.len() > 64 * 1024 * 1024 {
            return Err(io::Error::other("invalid container compilation artifact"));
        }
        fs::copy(candidate, output)?;
    }
    Ok(result)
}

/// Run a compiled program without mounting its host directory or inheriting host environment.
pub fn execute_binary(request: &ExecutionRequest) -> io::Result<ExecutionResult> {
    let binary = mount(Path::new(&request.program), "/program", true)?;
    let mut commands = vec!["/program"];
    commands.extend(request.args.iter().map(String::as_str));
    run_container(
        &[binary],
        &commands,
        request.stdin.as_deref(),
        &request.limits,
    )
}

fn mount(path: &Path, destination: &str, readonly: bool) -> io::Result<String> {
    let path = path.canonicalize()?;
    let path = path
        .to_str()
        .filter(|path| !path.contains([',', '"', '\n', '\r']))
        .ok_or_else(|| io::Error::other("path cannot be represented as a Docker bind mount"))?;
    let mode = if readonly { ",readonly" } else { "" };
    Ok(format!("type=bind,src={path},dst={destination}{mode}"))
}

fn capture(mut pipe: impl Read, limit: usize) -> io::Result<(String, bool)> {
    let mut output = Vec::new();
    let mut buffer = [0; 8192];
    let mut truncated = false;
    loop {
        let read = pipe.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        let keep = read.min(limit - output.len());
        output.extend_from_slice(&buffer[..keep]);
        truncated |= keep < read;
    }
    Ok((String::from_utf8_lossy(&output).into_owned(), truncated))
}

fn remove_container(name: &str) {
    if let Ok(mut child) = Command::new("docker")
        .args(["rm", "--force", name])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
    {
        if !matches!(child.wait_timeout(Duration::from_secs(5)), Ok(Some(_))) {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

fn run_container(
    mounts: &[String],
    command: &[&str],
    stdin: Option<&str>,
    limits: &ExecutionLimits,
) -> io::Result<ExecutionResult> {
    let started = Instant::now();
    let id = tempfile::Builder::new().prefix("codemorph-").tempdir()?;
    let name = id
        .path()
        .file_name()
        .and_then(|s| s.to_str())
        .ok_or_else(|| io::Error::other("invalid container name"))?;
    let image = std::env::var("CODEMORPH_SANDBOX_IMAGE").unwrap_or_else(|_| DEFAULT_IMAGE.into());
    if image.is_empty() || image.starts_with('-') {
        return Err(io::Error::other("invalid CODEMORPH_SANDBOX_IMAGE"));
    }
    let mut input = tempfile::tempfile()?;
    if let Some(value) = stdin {
        input.write_all(value.as_bytes())?;
        input.rewind()?;
    }
    let mut docker = Command::new("docker");
    docker.args([
        "run",
        "--rm",
        "--interactive",
        "--pull=never",
        "--name",
        name,
        "--network=none",
        "--read-only",
        "--cap-drop=ALL",
        "--security-opt=no-new-privileges",
        "--user=65534:65534",
        "--memory=1g",
        "--memory-swap=1g",
        "--cpus=1",
        "--pids-limit=64",
        "--ulimit",
        "fsize=67108864:67108864",
        "--log-driver=none",
        "--tmpfs",
        "/tmp:rw,nosuid,nodev,size=128m",
        "--workdir",
        "/tmp",
        "--env",
        "HOME=/tmp",
    ]);
    for mount in mounts {
        docker.args(["--mount", mount]);
    }
    let mut child = docker
        .arg(image)
        .args(command)
        .stdin(Stdio::from(input))
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| io::Error::other(format!("cannot start Docker sandbox: {error}")))?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| io::Error::other("missing stdout pipe"))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| io::Error::other("missing stderr pipe"))?;
    std::thread::scope(|scope| {
        let stdout = scope.spawn(|| capture(stdout, limits.max_stdout_bytes));
        let stderr = scope.spawn(|| capture(stderr, limits.max_stderr_bytes));
        let status = child.wait_timeout(Duration::from_millis(limits.timeout_ms));
        if !matches!(status, Ok(Some(_))) {
            remove_container(name);
            let _ = child.kill();
            let _ = child.wait();
            remove_container(name);
        }
        let status = status?;
        let (stdout, stdout_truncated) = stdout
            .join()
            .map_err(|_| io::Error::other("stdout capture failed"))??;
        let (stderr, stderr_truncated) = stderr
            .join()
            .map_err(|_| io::Error::other("stderr capture failed"))??;
        let exit_code = status.and_then(|status| status.code());
        if matches!(exit_code, Some(125..=127)) {
            return Err(io::Error::other(format!(
                "Docker sandbox execution failed: {}",
                stderr.trim()
            )));
        }
        if stdout_truncated || stderr_truncated {
            return Err(io::Error::other(
                "Docker sandbox output exceeded its capture limit",
            ));
        }
        Ok(ExecutionResult {
            exit_code,
            timed_out: status.is_none(),
            stdout,
            stderr,
            duration_ms: started.elapsed().as_millis() as u64,
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn binary_cannot_read_host_files_or_environment() {
        let host = tempfile::tempdir().unwrap();
        let sentinel = host.path().join("sentinel");
        fs::write(&sentinel, "outside container").unwrap();
        let source = host.path().join("main.rs");
        fs::write(
            &source,
            r#"fn main() {
                let sentinel = std::env::args().nth(1).unwrap();
                assert!(std::fs::read(sentinel).is_err());
                assert!(std::env::var_os("CODEMORPH_TEST_HOST_SECRET").is_none());
                assert!(std::fs::write("/outside-tmp", "read-only").is_err());
                println!("isolated");
            }"#,
        )
        .unwrap();
        let executable = host.path().join("program");
        let limits = ExecutionLimits {
            timeout_ms: 30_000,
            ..Default::default()
        };
        let compiled = compile("rustc", &[], &source, &executable, &limits).unwrap();
        assert_eq!(compiled.exit_code, Some(0), "{}", compiled.stderr);
        let result = execute_binary(&ExecutionRequest {
            program: executable.display().to_string(),
            args: vec![sentinel.display().to_string()],
            cwd: None,
            env: vec![("CODEMORPH_TEST_HOST_SECRET".into(), "synthetic".into())],
            stdin: None,
            limits,
        })
        .unwrap();
        assert_eq!(result.exit_code, Some(0), "{}", result.stderr);
        assert_eq!(result.stdout, "isolated\n");
    }
}
