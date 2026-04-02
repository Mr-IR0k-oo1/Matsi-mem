use anyhow::{bail, Result};
use std::process::{Command, Stdio};
use super::Executor;

/// Generic executor: runs `command [args...] <prompt>` as a subprocess.
/// The prompt is appended as the last argument.
///
/// Example: GenericExecutor::new("openai", vec!["api", "chat.completions.create", "-m", "gpt-4o", "-g"])
pub struct GenericExecutor {
    pub label:   String,
    pub command: String,
    pub args:    Vec<String>,
    /// If true, pass prompt via stdin instead of as a final argument
    pub stdin_mode: bool,
}

impl GenericExecutor {
    pub fn new(label: impl Into<String>, command: impl Into<String>, args: Vec<&str>) -> Self {
        Self {
            label: label.into(),
            command: command.into(),
            args: args.iter().map(|s| s.to_string()).collect(),
            stdin_mode: false,
        }
    }

    pub fn stdin(mut self) -> Self {
        self.stdin_mode = true;
        self
    }
}

impl Executor for GenericExecutor {
    fn name(&self) -> &str { &self.label }

    fn run(&self, prompt: &str) -> Result<String> {
        // Check binary exists
        if Command::new("which").arg(&self.command)
            .stdout(Stdio::null()).stderr(Stdio::null())
            .status().map(|s| !s.success()).unwrap_or(true)
        {
            bail!("'{}' not found in PATH", self.command);
        }

        let out = if self.stdin_mode {
            use std::io::Write;
            let mut child = Command::new(&self.command)
                .args(&self.args)
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()?;
            if let Some(ref mut stdin) = child.stdin {
                stdin.write_all(prompt.as_bytes())?;
            }
            child.wait_with_output()?
        } else {
            let mut cmd = Command::new(&self.command);
            cmd.args(&self.args);
            cmd.arg(prompt);
            cmd.output()?
        };

        if !out.status.success() {
            let err = String::from_utf8_lossy(&out.stderr);
            bail!("'{}' failed (exit {}): {}", self.command, out.status, err.trim());
        }

        let response = String::from_utf8_lossy(&out.stdout).trim().to_string();
        if response.is_empty() {
            Ok(format!("({} ran — no stdout output)", self.label))
        } else {
            Ok(response)
        }
    }
}
