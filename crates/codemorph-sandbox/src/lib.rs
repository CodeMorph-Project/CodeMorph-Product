pub mod container;

use codemorph_core::sandbox::{ExecutionRequest, ExecutionResult};
use std::io::Read;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};
use wait_timeout::ChildExt;

pub struct Sandbox;

impl Sandbox {
    pub fn execute(request: &ExecutionRequest) -> Result<ExecutionResult, std::io::Error> {
        let started = Instant::now();
        let mut command = Command::new(&request.program);
        command.args(&request.args);
        if let Some(cwd) = &request.cwd {
            command.current_dir(cwd);
        }
        for (k, v) in &request.env {
            command.env(k, v);
        }
        command.stdin(Stdio::piped());
        command.stdout(Stdio::piped());
        command.stderr(Stdio::piped());

        let mut child = command.spawn()?;
        if let Some(stdin) = &request.stdin {
            use std::io::Write;
            if let Some(mut p) = child.stdin.take() {
                let _ = p.write_all(stdin.as_bytes());
            }
        } else {
            let _ = child.stdin.take();
        }

        let stdout = child.stdout.take();
        let stderr = child.stderr.take();

        let timeout = Duration::from_millis(request.limits.timeout_ms);
        let status = child.wait_timeout(timeout)?;
        let timed_out = status.is_none();
        if timed_out {
            let _ = child.kill();
            let _ = child.wait();
        }

        let mut out_buf = Vec::new();
        let mut err_buf = Vec::new();
        if let Some(mut s) = stdout {
            let _ = s.read_to_end(&mut out_buf);
        }
        if let Some(mut s) = stderr {
            let _ = s.read_to_end(&mut err_buf);
        }

        let trim = |buf: Vec<u8>, limit: usize| -> String {
            let slice = if buf.len() > limit {
                &buf[..limit]
            } else {
                &buf
            };
            String::from_utf8_lossy(slice).to_string()
        };

        let exit_code = status.and_then(|s| s.code());
        Ok(ExecutionResult {
            exit_code,
            timed_out,
            stdout: trim(out_buf, request.limits.max_stdout_bytes),
            stderr: trim(err_buf, request.limits.max_stderr_bytes),
            duration_ms: started.elapsed().as_millis() as u64,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use codemorph_core::sandbox::ExecutionRequest;
    #[test]
    fn echo_works() {
        let req = ExecutionRequest {
            program: "/bin/echo".to_string(),
            args: vec!["hello".to_string()],
            cwd: None,
            env: vec![],
            stdin: None,
            limits: Default::default(),
        };
        let res = Sandbox::execute(&req).unwrap();
        assert_eq!(res.exit_code, Some(0));
        assert!(res.stdout.contains("hello"));
    }
    #[test]
    fn timeout_works() {
        let req = ExecutionRequest {
            program: "/bin/sleep".to_string(),
            args: vec!["10".to_string()],
            cwd: None,
            env: vec![],
            stdin: None,
            limits: codemorph_core::sandbox::ExecutionLimits {
                timeout_ms: 10,
                ..Default::default()
            },
        };
        let res = Sandbox::execute(&req).unwrap();
        assert!(res.timed_out);
    }
}
