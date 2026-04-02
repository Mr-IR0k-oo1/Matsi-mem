use anyhow::Result;
use std::sync::mpsc::{self, Receiver};
use std::thread;
use std::time::Instant;

use crate::context::{build, format_prompt, ContextOptions};
use crate::data::{AgentLog, Project, Session};
use crate::executor::{self, Model};
use crate::watcher::{start_watcher, WatchEvent};

// ── Tabs ──────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Tab {
    Run,      // prompt + model + response
    Agents,   // live external agent log feed
    Shims,    // shim installer / status
    Knowledge,// knowledge base browser
}

impl Tab {
    pub fn next(&self) -> Tab {
        match self {
            Tab::Run       => Tab::Agents,
            Tab::Agents    => Tab::Shims,
            Tab::Shims     => Tab::Knowledge,
            Tab::Knowledge => Tab::Run,
        }
    }
    pub fn prev(&self) -> Tab {
        match self {
            Tab::Run       => Tab::Knowledge,
            Tab::Agents    => Tab::Run,
            Tab::Shims     => Tab::Agents,
            Tab::Knowledge => Tab::Shims,
        }
    }
    pub fn label(&self) -> &'static str {
        match self {
            Tab::Run       => "[1] RUN",
            Tab::Agents    => "[2] AGENTS",
            Tab::Shims     => "[3] SHIMS",
            Tab::Knowledge => "[4] KNOWLEDGE",
        }
    }
}

// ── Focus ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Focus {
    Projects, Prompt, Context, Model, Response,
    AgentList, AgentDetail,
    ShimList,
    KnowledgeList, KnowledgeDetail,
}

// ── Run state ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RunState { Idle, Running, Done, Error(String) }

// ── Popups ────────────────────────────────────────────────────────────────────

pub enum Popup {
    None,
    NewProject  { name_buf: String, goal_buf: String, field: usize },
    AddKnowledge{ topic_buf: String, note_buf: String },
    Confirm     { message: String, on_yes: ConfirmAction },
    Output      { title: String, lines: Vec<String>, scroll: usize },
}

#[derive(Debug, Clone)]
pub enum ConfirmAction {
    DeleteProject(String),
    InstallShims,
    UninstallShims,
}

// ── Exec message ──────────────────────────────────────────────────────────────

pub enum ExecMsg {
    Done { response: String, duration_ms: u64 },
    Err(String),
}

// ── App ───────────────────────────────────────────────────────────────────────

pub struct App {
    pub tab:   Tab,
    pub focus: Focus,
    pub popup: Popup,

    // ── Projects ──────────────────────────────────────────────────────────────
    pub projects:         Vec<String>,
    pub project_idx:      usize,
    pub active_project:   Option<Project>,

    // ── Prompt / Run ──────────────────────────────────────────────────────────
    pub prompt:           String,
    pub cursor:           usize,
    pub ctx_project:      bool,
    pub ctx_sessions:     usize,
    pub ctx_knowledge:    bool,
    pub models:           Vec<Model>,
    pub model_idx:        usize,
    pub response:         String,
    pub response_scroll:  usize,
    pub run_state:        RunState,
    pub exec_rx:          Option<Receiver<ExecMsg>>,

    // ── External agents (live feed) ───────────────────────────────────────────
    pub agent_logs:       Vec<AgentLog>,   // most recent first, capped at 200
    pub agent_log_idx:    usize,
    pub agent_filter:     Option<String>,  // filter by agent name
    pub watch_rx:         Option<Receiver<WatchEvent>>,
    pub unread_count:     usize,           // new logs since last tab visit

    // ── Shims ─────────────────────────────────────────────────────────────────
    pub shim_statuses:    Vec<crate::watcher::ShimStatus>,
    pub shim_idx:         usize,
    pub shims_need_path:  bool,            // PATH line not yet in shell rc

    // ── Knowledge ─────────────────────────────────────────────────────────────
    pub knowledge_topics: Vec<String>,
    pub knowledge_idx:    usize,
    pub knowledge_detail: String,

    // ── Status bar ────────────────────────────────────────────────────────────
    pub status:           Option<(String, bool, Instant)>,

    pub should_quit: bool,
}

impl App {
    pub fn new() -> Result<Self> {
        let projects       = Project::list().unwrap_or_default();
        let active_project = projects.first()
            .and_then(|n| Project::load(n).ok());
        let agent_logs     = AgentLog::recent(200).unwrap_or_default();
        let shim_statuses  = crate::watcher::shim_status();
        let knowledge_topics = crate::data::Knowledge::list().unwrap_or_default();

        // Start file watcher
        let watch_rx = start_watcher().ok();

        // Check if shim dir is in PATH
        let shim_dir_str = crate::config::shims_dir().to_string_lossy().to_string();
        let path_env     = std::env::var("PATH").unwrap_or_default();
        let shims_need_path = !path_env.split(':').any(|p| p == shim_dir_str);

        Ok(App {
            tab:   Tab::Run,
            focus: Focus::Prompt,
            popup: Popup::None,

            projects,
            project_idx: 0,
            active_project,

            prompt: String::new(),
            cursor: 0,
            ctx_project:   true,
            ctx_sessions:  2,
            ctx_knowledge: false,
            models:        Model::presets(),
            model_idx:     0,
            response:      String::new(),
            response_scroll: 0,
            run_state:     RunState::Idle,
            exec_rx:       None,

            agent_logs,
            agent_log_idx: 0,
            agent_filter:  None,
            watch_rx,
            unread_count:  0,

            shim_statuses,
            shim_idx: 0,
            shims_need_path,

            knowledge_topics,
            knowledge_idx: 0,
            knowledge_detail: String::new(),

            status: None,
            should_quit: false,
        })
    }

    // ── Tick ─────────────────────────────────────────────────────────────────

    pub fn tick(&mut self) {
        // Clear expired status
        if let Some((_, _, until)) = &self.status {
            if Instant::now() > *until { self.status = None; }
        }

        // Poll executor
        self.poll_exec();

        // Poll file watcher
        self.poll_watcher();
    }

    fn poll_exec(&mut self) {
        if self.run_state != RunState::Running { return; }
        let Some(ref rx) = self.exec_rx else { return };
        match rx.try_recv() {
            Ok(ExecMsg::Done { response, duration_ms }) => {
                self.response    = response;
                self.run_state   = RunState::Done;
                self.exec_rx     = None;
                self.focus       = Focus::Response;
                self.set_status(format!("Done in {}ms — logged", duration_ms), false);
            }
            Ok(ExecMsg::Err(e)) => {
                self.run_state = RunState::Error(e.clone());
                self.exec_rx   = None;
                self.set_status(e, true);
            }
            Err(std::sync::mpsc::TryRecvError::Empty)        => {}
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                self.run_state = RunState::Error("executor thread died".into());
                self.exec_rx   = None;
            }
        }
    }

    fn poll_watcher(&mut self) {
        let Some(ref rx) = self.watch_rx else { return };
        // Drain all pending events
        loop {
            match rx.try_recv() {
                Ok(WatchEvent::NewLog(log)) => {
                    if self.tab != Tab::Agents {
                        self.unread_count += 1;
                    }
                    self.agent_logs.insert(0, log);
                    if self.agent_logs.len() > 200 {
                        self.agent_logs.truncate(200);
                    }
                }
                Ok(WatchEvent::Error(_)) => {}
                Err(_) => break,
            }
        }
    }

    // ── Status ────────────────────────────────────────────────────────────────

    pub fn set_status(&mut self, msg: impl Into<String>, error: bool) {
        self.status = Some((
            msg.into(),
            error,
            Instant::now() + std::time::Duration::from_secs(5),
        ));
    }

    // ── Tab switching ─────────────────────────────────────────────────────────

    pub fn switch_tab(&mut self, tab: Tab) {
        if tab == Tab::Agents {
            self.unread_count = 0;
        }
        self.tab   = tab;
        self.focus = match self.tab {
            Tab::Run       => Focus::Prompt,
            Tab::Agents    => Focus::AgentList,
            Tab::Shims     => Focus::ShimList,
            Tab::Knowledge => Focus::KnowledgeList,
        };
    }

    // ── Projects ─────────────────────────────────────────────────────────────

    pub fn reload_projects(&mut self) {
        self.projects    = Project::list().unwrap_or_default();
        self.project_idx = self.project_idx.min(self.projects.len().saturating_sub(1));
        self.reload_active();
    }

    pub fn reload_active(&mut self) {
        self.active_project = self.projects.get(self.project_idx)
            .and_then(|n| Project::load(n).ok());
    }

    // ── Run ───────────────────────────────────────────────────────────────────

    pub fn run(&mut self) {
        if self.run_state == RunState::Running { return; }
        if self.prompt.trim().is_empty() {
            self.set_status("Enter a prompt first", true);
            return;
        }
        let Some(ref project) = self.active_project.clone() else {
            self.set_status("Select or create a project first", true);
            return;
        };

        let opts = ContextOptions {
            include_project:  self.ctx_project,
            recent_sessions:  self.ctx_sessions,
            knowledge_query:  if self.ctx_knowledge { Some(self.prompt.clone()) } else { None },
        };

        let project      = project.clone();
        let prompt       = self.prompt.clone();
        let model        = self.models[self.model_idx].clone();
        let ctx          = match build(&project, &opts) {
            Ok(c) => c,
            Err(e) => { self.run_state = RunState::Error(e.to_string()); return; }
        };
        let full_prompt  = format_prompt(&ctx, &prompt);
        let ctx_summary  = ctx.summary.clone();
        let proj_name    = project.name.clone();
        let model_name   = model.display_name();

        let (tx, rx) = mpsc::channel();
        self.exec_rx   = Some(rx);
        self.run_state = RunState::Running;
        self.response.clear();
        self.response_scroll = 0;

        thread::spawn(move || {
            let start = Instant::now();
            match executor::run(&model, &full_prompt) {
                Ok(response) => {
                    let ms = start.elapsed().as_millis() as u64;
                    let sess = Session::new(&proj_name, &model_name, &prompt, &ctx_summary, &response, ms);
                    let _ = sess.save();
                    let _ = tx.send(ExecMsg::Done { response, duration_ms: ms });
                }
                Err(e) => { let _ = tx.send(ExecMsg::Err(e.to_string())); }
            }
        });
    }

    // ── Prompt editing ────────────────────────────────────────────────────────

    pub fn prompt_push(&mut self, c: char) {
        self.prompt.insert(self.cursor, c);
        self.cursor += c.len_utf8();
    }

    pub fn prompt_backspace(&mut self) {
        if self.cursor == 0 { return; }
        let mut c = self.cursor - 1;
        while !self.prompt.is_char_boundary(c) { c -= 1; }
        self.prompt.remove(c);
        self.cursor = c;
    }

    pub fn prompt_left(&mut self) {
        if self.cursor == 0 { return; }
        let mut c = self.cursor - 1;
        while !self.prompt.is_char_boundary(c) { c -= 1; }
        self.cursor = c;
    }

    pub fn prompt_right(&mut self) {
        if self.cursor >= self.prompt.len() { return; }
        let mut c = self.cursor + 1;
        while !self.prompt.is_char_boundary(c) { c += 1; }
        self.cursor = c;
    }

    // ── Model ─────────────────────────────────────────────────────────────────

    pub fn model_next(&mut self) { self.model_idx = (self.model_idx + 1) % self.models.len(); }
    pub fn model_prev(&mut self) {
        self.model_idx = if self.model_idx == 0 { self.models.len() - 1 } else { self.model_idx - 1 };
    }

    // ── Response scroll ───────────────────────────────────────────────────────

    pub fn response_down(&mut self) {
        let max = self.response.lines().count().saturating_sub(1);
        if self.response_scroll < max { self.response_scroll += 1; }
    }
    pub fn response_up(&mut self) {
        if self.response_scroll > 0 { self.response_scroll -= 1; }
    }

    // ── Agent log ─────────────────────────────────────────────────────────────

    pub fn filtered_logs(&self) -> Vec<&AgentLog> {
        self.agent_logs.iter()
            .filter(|l| {
                self.agent_filter.as_deref()
                    .map(|f| l.agent == f)
                    .unwrap_or(true)
            })
            .collect()
    }

    pub fn selected_log(&self) -> Option<&AgentLog> {
        let filtered = self.filtered_logs();
        filtered.get(self.agent_log_idx).copied()
    }

    pub fn agent_log_down(&mut self) {
        let max = self.filtered_logs().len().saturating_sub(1);
        if self.agent_log_idx < max { self.agent_log_idx += 1; }
    }
    pub fn agent_log_up(&mut self) {
        if self.agent_log_idx > 0 { self.agent_log_idx -= 1; }
    }

    // ── Knowledge ────────────────────────────────────────────────────────────

    pub fn reload_knowledge(&mut self) {
        self.knowledge_topics = crate::data::Knowledge::list().unwrap_or_default();
        self.knowledge_idx    = self.knowledge_idx.min(self.knowledge_topics.len().saturating_sub(1));
        self.refresh_knowledge_detail();
    }

    pub fn refresh_knowledge_detail(&mut self) {
        self.knowledge_detail = self.knowledge_topics
            .get(self.knowledge_idx)
            .and_then(|t| crate::data::Knowledge::load(t).ok())
            .map(|k| {
                let notes = k.notes.iter()
                    .enumerate()
                    .map(|(i, n)| format!("  {}. {}", i + 1, n))
                    .collect::<Vec<_>>()
                    .join("\n");
                format!("Topic: {}\n\nNotes:\n{}", k.topic, notes)
            })
            .unwrap_or_default();
    }

    pub fn knowledge_down(&mut self) {
        let max = self.knowledge_topics.len().saturating_sub(1);
        if self.knowledge_idx < max {
            self.knowledge_idx += 1;
            self.refresh_knowledge_detail();
        }
    }
    pub fn knowledge_up(&mut self) {
        if self.knowledge_idx > 0 {
            self.knowledge_idx -= 1;
            self.refresh_knowledge_detail();
        }
    }

    // ── Shims ────────────────────────────────────────────────────────────────

    pub fn reload_shim_status(&mut self) {
        self.shim_statuses = crate::watcher::shim_status();
        let shim_dir_str   = crate::config::shims_dir().to_string_lossy().to_string();
        let path_env       = std::env::var("PATH").unwrap_or_default();
        self.shims_need_path = !path_env.split(':').any(|p| p == shim_dir_str);
    }
}
