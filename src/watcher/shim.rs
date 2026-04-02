use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

use crate::config::{external_dir, shims_dir};

/// An agent whose CLI calls we want to intercept and log
pub struct AgentSpec {
    pub name:    &'static str,  // shim filename and log dir name
    pub bins:    &'static [&'static str],  // possible real binary names
    pub mode:    ShimMode,
}

#[derive(Clone, Copy)]
pub enum ShimMode {
    /// Pass prompt as last argument (e.g. ollama run model "prompt")
    ArgLast,
    /// Pass prompt via --print flag (Claude)
    PrintFlag,
    /// Pass prompt via -p flag (gemini)
    PFlag,
    /// Pass prompt via first positional arg
    FirstArg,
    /// Interactive — log invocation only, don't try to capture I/O
    Interactive,
}

pub const AGENTS: &[AgentSpec] = &[
    AgentSpec { name: "claude",  bins: &["claude"],        mode: ShimMode::PrintFlag },
    AgentSpec { name: "amp",     bins: &["amp"],            mode: ShimMode::FirstArg  },
    AgentSpec { name: "gemini",  bins: &["gemini"],         mode: ShimMode::PFlag     },
    AgentSpec { name: "vibe",    bins: &["vibe", "cursor"], mode: ShimMode::ArgLast   },
    AgentSpec { name: "aider",   bins: &["aider"],          mode: ShimMode::Interactive },
    AgentSpec { name: "copilot", bins: &["gh"],             mode: ShimMode::Interactive },
    AgentSpec { name: "mistral", bins: &["mistral"],        mode: ShimMode::ArgLast   },
    AgentSpec { name: "ollama",  bins: &["ollama"],         mode: ShimMode::ArgLast   },
];

/// Install shims for all known agents.
/// Returns (installed, skipped_already_shim, not_found) counts.
pub fn install_all() -> Result<(usize, usize, usize)> {
    let shim_dir = shims_dir();
    std::fs::create_dir_all(&shim_dir)?;

    let mut installed = 0usize;
    let mut already   = 0usize;
    let mut not_found = 0usize;

    for spec in AGENTS {
        match install_one(spec, &shim_dir) {
            Ok(true)  => installed += 1,
            Ok(false) => already   += 1,
            Err(_)    => not_found += 1,
        }
    }

    Ok((installed, already, not_found))
}

/// Install shim for one agent. Returns Ok(true) if newly installed,
/// Ok(false) if already a shim (idempotent), Err if real binary not found.
pub fn install_one(spec: &AgentSpec, shim_dir: &Path) -> Result<bool> {
    let shim_path = shim_dir.join(spec.name);

    // Idempotent: if shim already exists and it's ours, skip
    if shim_path.exists() {
        let content = std::fs::read_to_string(&shim_path).unwrap_or_default();
        if content.contains("matis-mem shim") {
            return Ok(false);
        }
    }

    // Find the real binary (must NOT be our shim dir)
    let real_bin = find_real_bin(spec.bins, shim_dir)
        .with_context(|| format!("no real binary found for {}", spec.name))?;

    let log_dir = external_dir().join(spec.name);
    let script = generate_shim(spec.name, &real_bin, &log_dir, spec.mode);

    // Write atomically
    let tmp = shim_path.with_extension("tmp");
    std::fs::write(&tmp, &script)?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&tmp)?.permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&tmp, perms)?;
    }

    std::fs::rename(tmp, &shim_path)?;
    Ok(true)
}

/// Remove all installed shims
pub fn uninstall_all() -> Result<usize> {
    let shim_dir = shims_dir();
    if !shim_dir.exists() { return Ok(0); }

    let mut removed = 0usize;
    for entry in std::fs::read_dir(&shim_dir)?.filter_map(|e| e.ok()) {
        let content = std::fs::read_to_string(entry.path()).unwrap_or_default();
        if content.contains("matis-mem shim") {
            std::fs::remove_file(entry.path())?;
            removed += 1;
        }
    }
    Ok(removed)
}

/// Find the real binary, excluding our shim directory from the search
fn find_real_bin(bins: &[&str], exclude_dir: &Path) -> Option<PathBuf> {
    let exclude = exclude_dir.to_string_lossy().to_string();
    for bin in bins {
        let output = std::process::Command::new("which")
            .arg("-a")
            .arg(bin)
            .output()
            .ok()?;
        for line in String::from_utf8_lossy(&output.stdout).lines() {
            let p = PathBuf::from(line.trim());
            if p.exists() && !p.to_string_lossy().starts_with(&exclude) {
                return Some(p);
            }
        }
    }
    None
}

/// Generate the bash shim script for one agent
fn generate_shim(
    name: &str,
    real_bin: &Path,
    log_dir: &Path,
    mode: ShimMode,
) -> String {
    let real = real_bin.display();
    let log  = log_dir.display();

    // The JSON serialisation helper — pure bash, no python required
    let json_escape = r#"
json_str() {
  local s="$1"
  s="${s//\\/\\\\}"
  s="${s//\"/\\\"}"
  s="${s//$'\n'/\\n}"
  s="${s//$'\t'/\\t}"
  printf '%s' "$s"
}"#;

    let project_detect = r#"
CWD=$(pwd)
GIT_ROOT=$(git rev-parse --show-toplevel 2>/dev/null || true)
if [[ -n "$GIT_ROOT" ]]; then
  PROJECT=$(basename "$GIT_ROOT")
else
  PROJECT=$(basename "$CWD")
fi"#;

    let write_log = format!(r#"
mkdir -p "{log_dir}"
TS=$(date +%Y%m%d_%H%M%S_%3N)
LOG_FILE="{log_dir}/$TS.json"
ISO=$(date -Iseconds)
cat > "$LOG_FILE" <<JSON
{{
  "id":          "$(json_str "$TS")",
  "agent":       "{name}",
  "cwd":         "$(json_str "$CWD")",
  "project":     "$(json_str "$PROJECT")",
  "args":        "$(json_str "$ARGS_STR")",
  "input":       "$(json_str "$INPUT")",
  "output":      "$(json_str "$OUTPUT")",
  "duration_ms": $DURATION,
  "exit_code":   $EXIT_CODE,
  "timestamp":   "$ISO",
  "capture":     "$CAPTURE_MODE"
}}
JSON"#,
        log_dir=log,
        name=name,
    );

    match mode {
        ShimMode::Interactive => format!(
r#"#!/usr/bin/env bash
# matis-mem shim for: {name}
{json_escape}
{project_detect}
ARGS_STR="$*"
INPUT=""
START=$(date +%s%3N 2>/dev/null || echo 0)
exec_and_time() {{
  "{real}" "$@"
  EXIT_CODE=$?
}}
exec_and_time "$@"
END=$(date +%s%3N 2>/dev/null || echo 0)
DURATION=$((END - START))
OUTPUT="(interactive)"
CAPTURE_MODE="interactive"
{write_log}
exit $EXIT_CODE
"#),

        ShimMode::PrintFlag => format!(
r#"#!/usr/bin/env bash
# matis-mem shim for: {name}
{json_escape}
{project_detect}
ARGS_STR="$*"
START=$(date +%s%3N 2>/dev/null || echo 0)

# Non-interactive: capture output
if [[ "$*" == *"--print"* ]] || [[ "$*" == *"-p "* ]] || [[ ! -t 0 ]]; then
  OUTPUT=$("{real}" "$@" 2>&1)
  EXIT_CODE=$?
  CAPTURE_MODE="full"
  INPUT=$(echo "$*" | sed 's/.*--print[= ]//;s/.*-p //')
else
  # Interactive session — pass through
  "{real}" "$@"
  EXIT_CODE=$?
  OUTPUT="(interactive)"
  CAPTURE_MODE="interactive"
  INPUT=""
fi
END=$(date +%s%3N 2>/dev/null || echo 0)
DURATION=$((END - START))
{write_log}
[[ "$CAPTURE_MODE" == "full" ]] && echo "$OUTPUT"
exit $EXIT_CODE
"#),

        ShimMode::PFlag => format!(
r#"#!/usr/bin/env bash
# matis-mem shim for: {name}
{json_escape}
{project_detect}
ARGS_STR="$*"
START=$(date +%s%3N 2>/dev/null || echo 0)

if [[ "$*" == *"-p "* ]] || [[ "$*" == *"--prompt"* ]]; then
  OUTPUT=$("{real}" "$@" 2>&1)
  EXIT_CODE=$?
  CAPTURE_MODE="full"
  INPUT=$(echo "$*" | sed 's/.*-p //;s/.*--prompt //')
else
  "{real}" "$@"
  EXIT_CODE=$?
  OUTPUT="(interactive)"
  CAPTURE_MODE="interactive"
  INPUT=""
fi
END=$(date +%s%3N 2>/dev/null || echo 0)
DURATION=$((END - START))
{write_log}
[[ "$CAPTURE_MODE" == "full" ]] && echo "$OUTPUT"
exit $EXIT_CODE
"#),

        ShimMode::ArgLast | ShimMode::FirstArg => format!(
r#"#!/usr/bin/env bash
# matis-mem shim for: {name}
{json_escape}
{project_detect}
ARGS_STR="$*"
INPUT="${{@: -1}}"
START=$(date +%s%3N 2>/dev/null || echo 0)
OUTPUT=$("{real}" "$@" 2>&1)
EXIT_CODE=$?
END=$(date +%s%3N 2>/dev/null || echo 0)
DURATION=$((END - START))
CAPTURE_MODE="full"
{write_log}
echo "$OUTPUT"
exit $EXIT_CODE
"#),
    }
}

/// Return the PATH export line the user needs to add to their shell rc
pub fn path_export_line() -> String {
    format!("export PATH=\"{}:$PATH\"", shims_dir().display())
}

/// Check which shims are currently installed and active in PATH
pub fn status() -> Vec<ShimStatus> {
    let shim_dir = shims_dir();
    AGENTS.iter().map(|spec| {
        let shim_path = shim_dir.join(spec.name);
        let installed = shim_path.exists() && {
            std::fs::read_to_string(&shim_path)
                .map(|c| c.contains("matis-mem shim"))
                .unwrap_or(false)
        };

        // Is our shim dir before the real binary in PATH?
        let active = if installed {
            std::process::Command::new("which")
                .arg(spec.name)
                .output()
                .map(|o| {
                    String::from_utf8_lossy(&o.stdout)
                        .trim()
                        .starts_with(&shim_dir.to_string_lossy().as_ref())
                })
                .unwrap_or(false)
        } else {
            false
        };

        let real_exists = find_real_bin(spec.bins, &shim_dir).is_some();

        ShimStatus {
            name:        spec.name,
            installed,
            active_in_path: active,
            real_exists,
        }
    }).collect()
}

pub struct ShimStatus {
    pub name:           &'static str,
    pub installed:      bool,
    pub active_in_path: bool,
    pub real_exists:    bool,
}
