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

## Project Philosophy and Design

This is not another wrapper around LLMs. matis-mem is a **deterministic, terminal-native AI execution environment** (an AI-OS in its architectural style) that enforces memory, context discipline, and reproducible execution across multiple AI backends.

---

## 💡 Philosophy

AI tools are stateless by default. That leads to:

*   repeated context injection
*   lost reasoning
*   inconsistent outputs
*   zero continuity

matis-mem fixes this by enforcing:

*   **persistent memory**
*   **deterministic context building**
*   **mandatory session logging**
*   **model-agnostic execution**

No magic. No hidden behavior. Everything is explicit and inspectable.

---

## 🧠 Core Concepts

### 1. Memory is First-Class

All state is stored locally:

```
~/.matis-mem/
├── projects/
├── sessions/
├── knowledge/
└── prompts/
```

*   **Projects** → long-lived context
*   **Sessions** → interaction history
*   **Knowledge** → reusable facts
*   **Prompts** → reusable templates

### 2. Deterministic Context

Context is constructed explicitly:

```
[PROJECT]
+ project.json

[RECENT SESSIONS]
+ last N sessions

[RELEVANT KNOWLEDGE]
+ keyword search (optional)
```

No hidden retrieval. No automatic hallucinated summaries.

### 3. Model-Agnostic Execution

All models are accessed through a single interface:

```rust
run(model, prompt)
```

Supported:

*   Ollama (local)
*   Gemini CLI

Adding new models requires implementing the `Executor` trait.

### 4. Mandatory Logging

Every execution produces a session log:

```json
{
  "timestamp": "...",
  "prompt": "...",
  "context_used": "...",
  "response": "..."
}
```

No logs = no memory = no system.

### 5. TUI = Control Surface

The terminal UI is strictly for:

*   selecting projects
*   editing prompts
*   toggling context
*   choosing models

All logic lives outside the UI.

---

## What it does

1.  **Memory** — stores projects, sessions, and knowledge as plain JSON in `~/.matis-mem/`
2.  **Context** — builds focused context: project + last N sessions + optional knowledge search
3.  **Execution** — routes prompts to any model (ollama, gemini) through one interface
4.  **Logging** — every run is saved automatically. No exceptions.

---

## 🗃️ Project Structure

```
matis-mem/
├── src/
│   ├── main.rs              # terminal setup, main loop
│   ├── config.rs            # ~/.matis-mem paths (OnceLock)
│   ├── error.rs             # typed errors
│
│   ├── data/
│   │   ├── project.rs       # Project CRUD + context export
│   │   ├── session.rs       # Session logging + retrieval
│   │   └── knowledge.rs     # Knowledge store + search
│
│   ├── context/
│   │   └── builder.rs       # context assembly logic
│
│   ├── executor/
│   │   ├── mod.rs           # Executor trait + Model enum
│   │   ├── ollama.rs        # ollama subprocess execution
│   │   └── gemini.rs        # gemini CLI execution
│
│   └── ui/
│       ├── app.rs           # app state + async execution
│       ├── events.rs        # keybindings
│       ├── render.rs        # layout rendering
│       └── theme.rs         # styling
```

---

## ⚙️ Installation

### Requirements

*   Rust (>= 1.75)
*   Ollama (optional but recommended)
*   Gemini CLI (optional)

### Build

```bash
git clone <repo>
cd matis-mem
bash install.sh
```

or build manually:

```bash
cargo build --release
```

### Fix for Unicode Dependency Issue

If build fails:

```bash
cargo update unicode-segmentation --precise 1.12.0
cargo build --release
```

This is required for Rust 1.75 compatibility.

---

## 🚀 Usage

Run:

```bash
./target/release/matis-mem
```

---

## 🖥️ TUI Overview

```
[ Project: millcheck ]

Prompt:
> improve parsing logic

Context:
[x] project
[x] last 2 sessions
[ ] knowledge

Model:
> ollama

[ RUN ]
```

---

## 🔄 Execution Flow

1.  Select project
2.  Enter prompt
3.  Build context
4.  Execute model
5.  Display response
6.  Log session automatically

---

## 🗺️ Data layout

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

---

## 🏗️ Context building

```
CONTEXT =
  [PROJECT]           always first, contains goal + constraints + decisions
+ [RECENT SESSIONS]   last N (default: 2) — prevents repeating the same question
+ [KNOWLEDGE]         keyword search across knowledge/ (optional, off by default)
```

Context is **explicit and minimal**. You can see exactly what's being injected
via the checkboxes in the Context panel before every run.

---

## 🛠️ Adding a new model

1.  Create `src/executor/mymodel.rs` implementing the `Executor` trait:

```rust
pub struct MyModelExecutor;

impl Executor for MyModelExecutor {
    fn name(&self) -> &str { "mymodel" }
    fn run(&self, prompt: &str) -> Result<String> {
        // spawn subprocess, return stdout
    }
}
```

2.  Add a variant to `Model` in `src/executor/mod.rs`
3.  Add it to `Model::all_presets()`
4.  Done — it appears in the model selector automatically

---

## ⚠️ Design Constraints

*   No implicit behavior
*   No automatic summarization
*   No hidden memory injection
*   No UI-driven logic

Everything must be:

*   explicit
*   deterministic
*   reproducible

---

## ✅ What This System Solves

*   Eliminates repeated context setup
*   Maintains continuity across sessions
*   Enables cross-model workflows
*   Provides inspectable AI interactions

---

## 💀 What It Does NOT Do

*   It does not improve bad prompts
*   It does not “think for you”
*   It does not replace engineering discipline

---

## 📈 Status

Core system implemented:

*   Memory layer
*   Context builder
*   Executor abstraction
*   TUI control surface
*   Session logging

---

## ⏭️ Next Steps

*   smarter retrieval (only if needed)
*   prompt templating
*   multi-agent chaining
*   benchmarking outputs

---

## 📄 License

MIT

---

## Final Note

This system is only useful if you actually use it consistently.

If you bypass it and go back to raw CLI usage, it becomes just another abandoned tool.