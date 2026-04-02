use anyhow::{bail, Result};
use std::process::{Command, Stdio};
use super::Executor;

pub struct MistralExecutor {
    pub model: String,
}

impl MistralExecutor {
    pub fn new(model: impl Into<String>) -> Self {
        Self { model: model.into() }
    }
}

impl Executor for MistralExecutor {
    fn name(&self) -> &str { "mistral" }

    fn run(&self, prompt: &str) -> Result<String> {
        // Try mistral CLI first, fall back to ollama mistral
        if Command::new("which").arg("mistral")
            .stdout(Stdio::null()).stderr(Stdio::null())
            .status().map(|s| s.success()).unwrap_or(false)
        {
            let out = Command::new("mistral")
                .args(["chat", "--no-stream", "-m", &self.model, prompt])
                .output()?;

            if out.status.success() {
                let r = String::from_utf8_lossy(&out.stdout).trim().to_string();
                if !r.is_empty() { return Ok(r); }
            }
        }

        // Fall back to ollama with mistral model
        if Command::new("which").arg("ollama")
            .stdout(Stdio::null()).stderr(Stdio::null())
            .status().map(|s| s.success()).unwrap_or(false)
        {
            let out = Command::new("ollama")
                .args(["run", &self.model, prompt])
                .output()?;

            if out.status.success() {
                let r = String::from_utf8_lossy(&out.stdout).trim().to_string();
                if !r.is_empty() { return Ok(r); }
            }
        }

        bail!(
            "mistral not available.\n\
             Option A: pip install mistralai && mistral setup\n\
             Option B: ollama pull {}", self.model
        )
    }
}
