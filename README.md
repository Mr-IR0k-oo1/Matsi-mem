# matis-mem

Terminal AI operating layer. Memory, context, execution — all in one TUI.

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

## What it does

1. **Memory** — stores projects, sessions, and knowledge as plain JSON in `~/.matis-mem/`
2. **Context** — builds focused context: project + last N sessions + optional knowledge search
3. **Execution** — routes prompts to any model (ollama, gemini) through one interface
4. **Logging** — every run is saved automatically. No exceptions.

## Install

```bash
git clone <repo> && cd matis-mem
bash install.sh
```

Requires: Rust 1.75+

## Models

| Model | Requirement |
|-------|-------------|
| `ollama/llama3` | `ollama pull llama3` |
| `ollama/mistral` | `ollama pull mistral` |
| `ollama/codellama` | `ollama pull codellama` |
| `gemini-cli` | `npm install -g @google/gemini-cli && gemini auth` |

## Keybindings

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

## Data layout

```
~/.matis-mem/
├── projects/
│   └── millcheck.json          # { name, goal, constraints, decisions, notes }
├── sessions/
│   └── millcheck/
│       └── 20260401_143022_001.json  # { prompt, context_summary, response, duration_ms, … }
├── knowledge/
│   └── pdf_parsing.json        # { topic, notes: [...], tags: [...] }
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

## Adding a new model

1. Create `src/executor/mymodel.rs` implementing the `Executor` trait:

```rust
pub struct MyModelExecutor;

impl Executor for MyModelExecutor {
    fn name(&self) -> &str { "mymodel" }
    fn run(&self, prompt: &str) -> Result<String> {
        // spawn subprocess, return stdout
    }
}
```

2. Add a variant to `Model` in `src/executor/mod.rs`
3. Add it to `Model::all_presets()`
4. Done — it appears in the model selector automatically

## Design rules (don't break these)

- **Deterministic context** — no magic injection. What you see in the panel is what gets sent.
- **Small context > big context** — default is project + 2 sessions. Raise it only when needed.
- **Model-agnostic** — `executor::run(model, prompt)` is the only call site.
- **Logging is mandatory** — `Session::save()` is called before `ExecMsg::Done` is sent.
- **TUI = control, not logic** — `render.rs` only reads state. All logic is in `app.rs`, `context/`, `executor/`.
