use anyhow::{bail, Result};
use std::process::{Command, Stdio};
use super::Executor;

pub struct VibeExecutor;

impl VibeExecutor {
    pub fn new() -> Self { Self }

    fn find_bin() -> Option<String> {
        for name in &["vibe", "cursor"] {
            if Command::new("which").arg(name)
                .stdout(Stdio::null()).stderr(Stdio::null())
                .status().map(|s| s.success()).unwrap_or(false)
            {
                return Some(name.to_string());
            }
        }
        None
    }
}

impl Executor for VibeExecutor {
    fn name(&self) -> &str { "vibe" }

    fn run(&self, prompt: &str) -> Result<String> {
        let bin = Self::find_bin()
            .ok_or_else(|| anyhow::anyhow!(
                "vibe/cursor not found in PATH.\n\
                 Install Vibe CLI or Cursor and ensure it's in PATH."
            ))?;

        // Most Vibe/Cursor CLI implementations accept --message or positional arg
        let out = Command::new(&bin)
            .args(["--message", prompt])
            .output()?;

        if !out.status.success() {
            // Try positional
            let out2 = Command::new(&bin).arg(prompt).output()?;
            if out2.status.success() {
                let r = String::from_utf8_lossy(&out2.stdout).trim().to_string();
                if !r.is_empty() { return Ok(r); }
            }
            bail!("vibe failed: {}", String::from_utf8_lossy(&out.stderr).trim());
        }

        let response = String::from_utf8_lossy(&out.stdout).trim().to_string();
        if response.is_empty() {
            Ok("(vibe ran — no stdout captured; check editor for output)".into())
        } else {
            Ok(response)
        }
    }
}
