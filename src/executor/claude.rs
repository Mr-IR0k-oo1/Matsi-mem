use anyhow::{bail, Result};
use std::process::{Command, Stdio};
use super::Executor;

pub struct ClaudeExecutor {
    /// If true, use `claude --print` (non-interactive, captures output)
    /// If false, launch interactive claude code session
    pub non_interactive: bool,
}

impl ClaudeExecutor {
    pub fn new(non_interactive: bool) -> Self { Self { non_interactive } }

    fn find_bin() -> Result<String> {
        for name in &["claude", "claude-code"] {
            if Command::new("which").arg(name)
                .stdout(Stdio::null()).stderr(Stdio::null())
                .status().map(|s| s.success()).unwrap_or(false)
            {
                return Ok(name.to_string());
            }
        }
        bail!("claude not found in PATH.\nInstall: npm install -g @anthropic-ai/claude-code")
    }
}

impl Executor for ClaudeExecutor {
    fn name(&self) -> &str {
        if self.non_interactive { "claude (print)" } else { "claude code" }
    }

    fn run(&self, prompt: &str) -> Result<String> {
        let bin = Self::find_bin()?;

        if self.non_interactive {
            // claude --print "prompt" — captures full output
            let out = Command::new(&bin)
                .args(["--print", prompt])
                .output()?;

            if !out.status.success() {
                let err = String::from_utf8_lossy(&out.stderr);
                bail!("claude --print failed: {}", err.trim());
            }
            let response = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if response.is_empty() { bail!("claude returned empty response"); }
            Ok(response)
        } else {
            // Interactive — pipe prompt via stdin, capture stdout
            let mut child = Command::new(&bin)
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()?;

            use std::io::Write;
            if let Some(ref mut stdin) = child.stdin {
                stdin.write_all(prompt.as_bytes())?;
            }

            let out = child.wait_with_output()?;
            if !out.status.success() {
                let err = String::from_utf8_lossy(&out.stderr);
                bail!("claude failed: {}", err.trim());
            }
            let response = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if response.is_empty() {
                Ok("(Claude ran but produced no text output — use --print mode for capture)".into())
            } else {
                Ok(response)
            }
        }
    }
}
