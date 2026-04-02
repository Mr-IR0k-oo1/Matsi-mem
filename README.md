# Matis-MEM

Terminal AI memory system. Persistent context, multi-model execution, and session logging in one TUI.

```
┌ ◆ matis-mem  project: millcheck  model: ollama/llama3  ◉ done ──────────────┐
│                    │                                                       │
│  PROJECTS          │  PROMPT                                               │
│ ▶ millcheck        │  fix the edge case in the MTC parser where            │
│   api-gateway      │  null dates cause a panic▌                            │
│                    ├───────────────────────────────────────────────────────│
│                    │  CONTEXT              │  MODEL                        │
│                    │  [x] project context  │  ▶ ollama/llama3              │
│                    │  [x] last 2 sessions  │    ollama/mistral             │
│                    │  [ ] knowledge search │    ollama/codellama           │
│                    │                       │    gemini-cli                 │
│                    │  Ctrl+R / F5 = RUN    │                               │
│                    ├───────────────────────────────────────────────────────│
│                    │  RESPONSE                                             │
│                    │  The panic occurs because `parse_date()` returns      │
│                    │  `Option<Date>` but the caller uses `.unwrap()`…      │
└────────────────────┴───────────────────────────────────────────────────────┘
 [j/k] scroll  [Tab] → prompt  [y] copy  [Ctrl+R] run again
```

## Quick start

### Install

```bash
git clone <repo> && cd Matis-MEM
bash install.sh
```

Requires: Rust 1.75+

### Run

```bash
matis-mem
```

## What it does

1. **Memory** — stores projects, sessions, and knowledge as plain JSON in `~/.matis-mem/`
2. **Context** — builds focused context: project + last N sessions + optional knowledge search
3. **Execution** — routes prompts to multiple models (ollama, gemini, claude, mistral, amp) through unified interface
4. **Logging** — every run is saved automatically with response metadata and duration tracking
5. **Watcher** — monitors tool execution logs and indexes responses for context retrieval

## Architecture

### Core modules

- **`executor/`** — Model execution (ollama, gemini, claude, mistral, amp, vibe)
- **`context/`** — Context building: projects + sessions + knowledge
- **`data/`** — Persistent storage: projects, sessions, knowledge, agent logs
- **`ui/`** — TUI rendering with ratatui
- **`watcher/`** — Log monitoring and agent response indexing
- **`config.rs`** — Config management and env vars
- **`error.rs`** — Custom error types

## Data layout

```
~/.matis-mem/
├── projects/
│   └── millcheck.json          # { name, goal, constraints, decisions, notes }
├── sessions/
│   └── millcheck/
│       └── 20260401_143022_001.json  # { prompt, context_summary, response, duration_ms, model, … }
├── knowledge/
│   └── pdf_parsing.json        # { topic, notes: [...], tags: [...] }
├── agent_logs/
│   └── 2026-04-01.jsonl        # { timestamp, thread, agent_output, indexed_response, … }
└── prompts/                    # reserved for saved prompt templates
```

## Context building

```
CONTEXT =
  [PROJECT]           always first, contains goal + constraints + decisions
+ [RECENT SESSIONS]   last N (default: 2) — prevents repeating the same question
+ [KNOWLEDGE]         keyword search across knowledge/ (optional, off by default)
```

Context is **explicit and minimal**. You can see exactly what's being injected
via the checkboxes in the Context panel before every run.

## Usage

### Keybindings

| Key | Action |
|-----|--------|
| `Tab` / `Shift+Tab` | Cycle focus between panels |
| `Ctrl+R` / `F5` | Run prompt |
| `Enter` (in prompt) | Run prompt |
| `Shift+Enter` | Newline in prompt |
| `Ctrl+N` | New project |
| `Ctrl+K` | Add knowledge entry |
| `j/k` or `↑/↓` | Navigate lists / scroll response |
| `Space` | Toggle project context on/off |
| `-` / `+` | Decrease/increase recent sessions count |
| `k` (in context panel) | Toggle knowledge search |
| `c` (in response) | Clear and start new prompt |
| `q` / `Ctrl+C` | Quit |

## Supported Models

| Model | Executor | Requirement |
|-------|----------|-------------|
| `ollama/*` | Generic ollama | `ollama serve` (running locally) |
| `gemini` | Google Gemini | `gemini auth` configured |
| `claude` | Anthropic Claude | `ANTHROPIC_API_KEY` env var |
| `mistral` | Mistral API | `MISTRAL_API_KEY` env var |
| `amp` | Amp Agent | `amp` CLI installed and authenticated |
| `vibe` | Vibe (local) | Custom local model endpoint |

## Development

### Adding a new model

1. Create `src/executor/mymodel.rs` implementing the `Executor` trait:

```rust
use crate::executor::Executor;

pub struct MyModelExecutor;

impl Executor for MyModelExecutor {
    fn name(&self) -> &str { "mymodel" }
    fn run(&self, prompt: &str) -> Result<String> {
        // spawn subprocess, return stdout
        unimplemented!()
    }
}
```

2. Add module to `src/executor/mod.rs`:
   ```rust
   mod mymodel;
   pub use mymodel::MyModelExecutor;
   ```

3. Add variant to `Model` enum and `impl Model::all_presets()`
4. Done — appears in the model selector automatically

### Design rules (don't break these)

- **Deterministic context** — no magic injection. What you see in the panel is what gets sent.
- **Small context > big context** — default is project + 2 sessions. Raise it only when needed.
- **Single executor call site** — all model invocations go through unified interface
- **Logging is mandatory** — sessions and agent logs saved before UI confirmation
- **TUI = control, not logic** — UI only reads/renders state. Logic in `app.rs`, `context/`, `executor/`, `watcher/`
- **Extensible executors** — adding models requires only implementing `Executor` trait
