use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::config::external_dir;

/// How an external agent session was captured
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CaptureMode {
    /// Full stdin/stdout captured (non-interactive call e.g. `claude --print "..."`)
    Full,
    /// Interactive session — we captured invocation + cwd + project, not I/O
    Interactive,
    /// Amp-style task run — captured task description + result
    Task,
}

impl std::fmt::Display for CaptureMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CaptureMode::Full        => write!(f, "full"),
            CaptureMode::Interactive => write!(f, "interactive"),
            CaptureMode::Task        => write!(f, "task"),
        }
    }
}

/// One external agent session logged by a shim
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentLog {
    pub id:          String,        // timestamp-based filename stem
    pub agent:       String,        // "claude", "amp", "vibe", etc.
    pub cwd:         String,        // working directory when called
    pub project:     String,        // detected from git root basename (or cwd basename)
    pub args:        String,        // raw CLI args as a single string
    pub input:       String,        // stdin or prompt argument
    pub output:      String,        // stdout (or "(interactive)" if not captured)
    pub duration_ms: u64,
    pub exit_code:   i32,
    pub timestamp:   String,        // RFC 3339
    pub capture:     CaptureMode,
}

impl AgentLog {
    pub fn path_for(agent: &str, id: &str) -> PathBuf {
        external_dir().join(agent).join(format!("{}.json", id))
    }

    pub fn save(&self) -> Result<()> {
        let dir = external_dir().join(&self.agent);
        std::fs::create_dir_all(&dir)?;
        let p = dir.join(format!("{}.json", self.id));
        let tmp = p.with_extension("tmp");
        std::fs::write(&tmp, serde_json::to_string_pretty(self)?)?;
        std::fs::rename(tmp, p)?;
        Ok(())
    }

    pub fn load(path: &std::path::Path) -> Result<Self> {
        let raw = std::fs::read_to_string(path)?;
        Ok(serde_json::from_str(&raw)?)
    }

    /// Load last N logs across ALL agents, sorted newest first
    pub fn recent(n: usize) -> Result<Vec<Self>> {
        let root = external_dir();
        if !root.exists() { return Ok(vec![]); }

        let mut all: Vec<(std::time::SystemTime, PathBuf)> = Vec::new();

        for agent_entry in std::fs::read_dir(&root)?.filter_map(|e| e.ok()) {
            if !agent_entry.path().is_dir() { continue; }
            for log_entry in std::fs::read_dir(agent_entry.path())?.filter_map(|e| e.ok()) {
                let p = log_entry.path();
                if p.extension().and_then(|x| x.to_str()) == Some("json") {
                    let mtime = p.metadata()
                        .and_then(|m| m.modified())
                        .unwrap_or(std::time::UNIX_EPOCH);
                    all.push((mtime, p));
                }
            }
        }

        all.sort_by(|a, b| b.0.cmp(&a.0));

        let mut logs = Vec::new();
        for (_, path) in all.into_iter().take(n) {
            if let Ok(log) = Self::load(&path) {
                logs.push(log);
            }
        }
        Ok(logs)
    }

    /// Load all logs for a specific agent, newest first
    pub fn for_agent(agent: &str, limit: usize) -> Result<Vec<Self>> {
        let dir = external_dir().join(agent);
        if !dir.exists() { return Ok(vec![]); }

        let mut files: Vec<PathBuf> = std::fs::read_dir(&dir)?
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().and_then(|x| x.to_str()) == Some("json"))
            .map(|e| e.path())
            .collect();
        files.sort_by(|a, b| b.cmp(a));

        let mut logs = Vec::new();
        for path in files.into_iter().take(limit) {
            if let Ok(log) = Self::load(&path) {
                logs.push(log);
            }
        }
        Ok(logs)
    }

    /// List all agents that have logs
    pub fn known_agents() -> Vec<String> {
        let root = external_dir();
        if !root.exists() { return vec![]; }
        let mut agents: Vec<String> = std::fs::read_dir(&root)
            .into_iter()
            .flatten()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().is_dir())
            .map(|e| e.file_name().to_string_lossy().to_string())
            .collect();
        agents.sort();
        agents
    }

    /// Short one-line summary for list display
    pub fn summary(&self) -> String {
        let input_preview: String = self.input.chars().take(60).collect();
        let input_str = if input_preview.is_empty() {
            self.args.chars().take(60).collect::<String>()
        } else {
            input_preview
        };
        format!("[{}] {} · {} · {}ms",
            &self.timestamp[..16],
            self.agent,
            input_str,
            self.duration_ms)
    }
}
