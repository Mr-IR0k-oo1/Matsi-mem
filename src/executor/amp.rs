use anyhow::{bail, Result};
use std::process::{Command, Stdio};
use super::Executor;

pub struct AmpExecutor;

impl AmpExecutor {
    pub fn new() -> Self { Self }
}

impl Executor for AmpExecutor {
    fn name(&self) -> &str { "amp" }

    fn run(&self, prompt: &str) -> Result<String> {
        if Command::new("which").arg("amp")
            .stdout(Stdio::null()).stderr(Stdio::null())
            .status().map(|s| !s.success()).unwrap_or(true)
        {
            bail!("amp not found in PATH.\nInstall from: https://ampcode.com");
        }

        // amp run "task description" — non-interactive task execution
        let out = Command::new("amp")
            .args(["run", prompt])
            .output()?;

        if !out.status.success() {
            let err = String::from_utf8_lossy(&out.stderr);
            bail!("amp failed: {}", err.trim());
        }

        let response = String::from_utf8_lossy(&out.stdout).trim().to_string();
        if response.is_empty() {
            Ok("(amp ran — check terminal for interactive output)".into())
        } else {
            Ok(response)
        }
    }
}
