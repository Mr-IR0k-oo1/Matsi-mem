use crossterm::event::{Event, KeyCode, KeyModifiers};
use crate::data::{Knowledge, Project};
use crate::watcher::shim;
use super::app::{App, ConfirmAction, Focus, Popup, RunState, Tab};

pub fn handle(event: &Event, app: &mut App) {
    let Event::Key(key) = event else { return };

    // Ctrl+C always quits
    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
        app.should_quit = true;
        return;
    }

    // Popup routing
    if !matches!(app.popup, Popup::None) {
        handle_popup(key.code, key.modifiers, app);
        return;
    }

    // Global Ctrl shortcuts
    if key.modifiers.contains(KeyModifiers::CONTROL) {
        match key.code {
            KeyCode::Char('r') => { app.run(); return; }
            KeyCode::Char('n') => {
                app.popup = Popup::NewProject {
                    name_buf: String::new(), goal_buf: String::new(), field: 0,
                };
                return;
            }
            KeyCode::Char('k') => {
                app.popup = Popup::AddKnowledge {
                    topic_buf: String::new(), note_buf: String::new(),
                };
                return;
            }
            _ => {}
        }
    }

    // F5 = run
    if key.code == KeyCode::F(5) { app.run(); return; }

    // Tab switching: 1–4
    match key.code {
        KeyCode::Char('1') => { app.switch_tab(Tab::Run);       return; }
        KeyCode::Char('2') => { app.switch_tab(Tab::Agents);    return; }
        KeyCode::Char('3') => { app.switch_tab(Tab::Shims);     return; }
        KeyCode::Char('4') => { app.switch_tab(Tab::Knowledge); return; }
        KeyCode::Tab => {
            app.focus = match app.focus {
                Focus::Projects      => Focus::Prompt,
                Focus::Prompt        => Focus::Context,
                Focus::Context       => Focus::Model,
                Focus::Model         => Focus::Response,
                Focus::Response      => Focus::Projects,
                Focus::AgentList     => Focus::AgentDetail,
                Focus::AgentDetail   => Focus::AgentList,
                Focus::ShimList      => Focus::ShimList,
                Focus::KnowledgeList => Focus::KnowledgeDetail,
                Focus::KnowledgeDetail => Focus::KnowledgeList,
            };
            return;
        }
        KeyCode::BackTab => {
            app.focus = match app.focus {
                Focus::Projects      => Focus::Response,
                Focus::Prompt        => Focus::Projects,
                Focus::Context       => Focus::Prompt,
                Focus::Model         => Focus::Context,
                Focus::Response      => Focus::Model,
                Focus::AgentList     => Focus::AgentDetail,
                Focus::AgentDetail   => Focus::AgentList,
                Focus::ShimList      => Focus::ShimList,
                Focus::KnowledgeList => Focus::KnowledgeDetail,
                Focus::KnowledgeDetail => Focus::KnowledgeList,
            };
            return;
        }
        KeyCode::Char('q') if !matches!(app.focus, Focus::Prompt) => {
            app.should_quit = true;
            return;
        }
        _ => {}
    }

    match app.tab {
        Tab::Run       => handle_run(key.code, key.modifiers, app),
        Tab::Agents    => handle_agents(key.code, app),
        Tab::Shims     => handle_shims(key.code, app),
        Tab::Knowledge => handle_knowledge(key.code, key.modifiers, app),
    }
}

// ── Popup ─────────────────────────────────────────────────────────────────────

fn handle_popup(code: KeyCode, mods: KeyModifiers, app: &mut App) {
    match &mut app.popup {
        Popup::NewProject { name_buf, goal_buf, field } => {
            let f = *field;
            match code {
                KeyCode::Esc    => app.popup = Popup::None,
                KeyCode::Tab    => *field = if f == 0 { 1 } else { 0 },
                KeyCode::Enter  => {
                    let name = name_buf.trim().to_string();
                    let goal = goal_buf.trim().to_string();
                    if !name.is_empty() && !goal.is_empty() {
                        let proj = Project::new(&name, &goal);
                        match proj.save() {
                            Ok(_)  => app.set_status(format!("Project '{}' created", name), false),
                            Err(e) => app.set_status(format!("Error: {}", e), true),
                        }
                        app.popup = Popup::None;
                        app.reload_projects();
                        if let Some(idx) = app.projects.iter().position(|p| p == &name) {
                            app.project_idx = idx;
                            app.reload_active();
                        }
                    }
                }
                KeyCode::Backspace => {
                    if f == 0 { name_buf.pop(); } else { goal_buf.pop(); }
                }
                KeyCode::Char(c) if mods == KeyModifiers::NONE || mods == KeyModifiers::SHIFT => {
                    if f == 0 { name_buf.push(c); } else { goal_buf.push(c); }
                }
                _ => {}
            }
        }

        Popup::AddKnowledge { topic_buf, note_buf } => {
            match code {
                KeyCode::Esc   => app.popup = Popup::None,
                KeyCode::Enter => {
                    let topic = topic_buf.trim().to_string();
                    let note  = note_buf.trim().to_string();
                    if !topic.is_empty() && !note.is_empty() {
                        let mut k = Knowledge::load(&topic)
                            .unwrap_or_else(|_| Knowledge::new(&topic));
                        k.notes.push(note);
                        match k.save() {
                            Ok(_)  => {
                                app.set_status(format!("Saved knowledge: {}", topic), false);
                                app.reload_knowledge();
                            }
                            Err(e) => app.set_status(format!("Error: {}", e), true),
                        }
                        app.popup = Popup::None;
                    }
                }
                KeyCode::Backspace => { note_buf.pop(); }
                KeyCode::Char(c) if mods == KeyModifiers::NONE || mods == KeyModifiers::SHIFT => {
                    // Simple: type into note until topic is non-empty; then type into note
                    if topic_buf.is_empty() {
                        topic_buf.push(c);
                    } else {
                        note_buf.push(c);
                    }
                }
                _ => {}
            }
        }

        Popup::Confirm { on_yes, .. } => {
            match code {
                KeyCode::Esc | KeyCode::Char('n') => app.popup = Popup::None,
                KeyCode::Enter | KeyCode::Char('y') => {
                    let action = on_yes.clone();
                    app.popup  = Popup::None;
                    match action {
                        ConfirmAction::DeleteProject(name) => {
                            match Project::delete(&name) {
                                Ok(_)  => {
                                    app.set_status(format!("Deleted '{}'", name), false);
                                    app.reload_projects();
                                }
                                Err(e) => app.set_status(format!("Error: {}", e), true),
                            }
                        }
                        ConfirmAction::InstallShims => {
                            match shim::install_all() {
                                Ok((installed, already, not_found)) => {
                                    let lines = vec![
                                        format!("  ✓ Installed: {}", installed),
                                        format!("  ─ Already shims: {}", already),
                                        format!("  ○ Binary not found: {}", not_found),
                                        String::new(),
                                        format!("  Add to shell rc: {}", shim::path_export_line()),
                                        "  Then restart your terminal or: source ~/.zshrc".into(),
                                    ];
                                    app.popup = Popup::Output {
                                        title: "Shim Install".into(),
                                        lines,
                                        scroll: 0,
                                    };
                                    app.reload_shim_status();
                                }
                                Err(e) => app.set_status(format!("Install failed: {}", e), true),
                            }
                        }
                        ConfirmAction::UninstallShims => {
                            match shim::uninstall_all() {
                                Ok(n) => {
                                    app.set_status(format!("Removed {} shim(s)", n), false);
                                    app.reload_shim_status();
                                }
                                Err(e) => app.set_status(format!("Error: {}", e), true),
                            }
                        }
                    }
                }
                _ => {}
            }
        }

        Popup::Output { scroll, lines, .. } => {
            match code {
                KeyCode::Char('j') | KeyCode::Down => {
                    let max = lines.len().saturating_sub(1);
                    if *scroll < max { *scroll += 1; }
                }
                KeyCode::Char('k') | KeyCode::Up => {
                    if *scroll > 0 { *scroll -= 1; }
                }
                _ => app.popup = Popup::None,
            }
        }

        Popup::None => {}
    }
}

// ── Run tab ───────────────────────────────────────────────────────────────────

fn handle_run(code: KeyCode, mods: KeyModifiers, app: &mut App) {
    match app.focus {
        Focus::Projects  => handle_projects(code, app),
        Focus::Prompt    => handle_prompt(code, mods, app),
        Focus::Context   => handle_context(code, app),
        Focus::Model     => handle_model(code, app),
        Focus::Response  => handle_response(code, app),
        _ => {}
    }
}

fn handle_projects(code: KeyCode, app: &mut App) {
    match code {
        KeyCode::Char('j') | KeyCode::Down => {
            if app.project_idx + 1 < app.projects.len() {
                app.project_idx += 1;
                app.reload_active();
            }
        }
        KeyCode::Char('k') | KeyCode::Up => {
            if app.project_idx > 0 {
                app.project_idx -= 1;
                app.reload_active();
            }
        }
        KeyCode::Enter => { app.reload_active(); app.focus = Focus::Prompt; }
        KeyCode::Char('n') => {
            app.popup = Popup::NewProject {
                name_buf: String::new(), goal_buf: String::new(), field: 0,
            };
        }
        KeyCode::Char('d') | KeyCode::Delete => {
            if let Some(name) = app.projects.get(app.project_idx).cloned() {
                app.popup = Popup::Confirm {
                    message: format!("Delete project '{}'?", name),
                    on_yes: ConfirmAction::DeleteProject(name),
                };
            }
        }
        _ => {}
    }
}

fn handle_prompt(code: KeyCode, mods: KeyModifiers, app: &mut App) {
    match code {
        KeyCode::Enter => {
            if mods.contains(KeyModifiers::SHIFT) {
                app.prompt_push('\n');
            } else {
                app.run();
            }
        }
        KeyCode::Char(c) if mods == KeyModifiers::NONE || mods == KeyModifiers::SHIFT => {
            app.prompt_push(c);
        }
        KeyCode::Backspace => app.prompt_backspace(),
        KeyCode::Left      => app.prompt_left(),
        KeyCode::Right     => app.prompt_right(),
        _ => {}
    }
}

fn handle_context(code: KeyCode, app: &mut App) {
    match code {
        KeyCode::Char(' ')               => app.ctx_project   = !app.ctx_project,
        KeyCode::Char('+') | KeyCode::Char('=') => {
            if app.ctx_sessions < 10 { app.ctx_sessions += 1; }
        }
        KeyCode::Char('-') => {
            if app.ctx_sessions > 0 { app.ctx_sessions -= 1; }
        }
        KeyCode::Char('k') => app.ctx_knowledge = !app.ctx_knowledge,
        _ => {}
    }
}

fn handle_model(code: KeyCode, app: &mut App) {
    match code {
        KeyCode::Char('j') | KeyCode::Down => app.model_next(),
        KeyCode::Char('k') | KeyCode::Up   => app.model_prev(),
        _ => {}
    }
}

fn handle_response(code: KeyCode, app: &mut App) {
    match code {
        KeyCode::Char('j') | KeyCode::Down => app.response_down(),
        KeyCode::Char('k') | KeyCode::Up   => app.response_up(),
        KeyCode::Char('g')                  => app.response_scroll = 0,
        KeyCode::Char('G')                  => {
            app.response_scroll = app.response.lines().count().saturating_sub(1);
        }
        KeyCode::Char('c') if app.run_state == RunState::Done => {
            app.prompt.clear();
            app.cursor = 0;
            app.run_state = RunState::Idle;
            app.response.clear();
            app.focus = Focus::Prompt;
        }
        _ => {}
    }
}

// ── Agents tab ────────────────────────────────────────────────────────────────

fn handle_agents(code: KeyCode, app: &mut App) {
    match app.focus {
        Focus::AgentList => match code {
            KeyCode::Char('j') | KeyCode::Down => app.agent_log_down(),
            KeyCode::Char('k') | KeyCode::Up   => app.agent_log_up(),
            KeyCode::Char('r') => {
                app.agent_logs = crate::data::AgentLog::recent(200).unwrap_or_default();
                app.set_status("Refreshed agent logs", false);
            }
            KeyCode::Char('a') => { app.agent_filter = None; app.agent_log_idx = 0; }
            KeyCode::Char('f') => {
                // Cycle through known agents as filter
                let agents = crate::data::AgentLog::known_agents();
                if agents.is_empty() { return; }
                let next = match &app.agent_filter {
                    None => agents.first().cloned(),
                    Some(cur) => {
                        let pos = agents.iter().position(|a| a == cur).unwrap_or(0);
                        if pos + 1 < agents.len() {
                            Some(agents[pos + 1].clone())
                        } else {
                            None // wrap to "all"
                        }
                    }
                };
                app.agent_filter  = next;
                app.agent_log_idx = 0;
            }
            _ => {}
        }
        Focus::AgentDetail => match code {
            KeyCode::Char('j') | KeyCode::Down => app.agent_log_down(),
            KeyCode::Char('k') | KeyCode::Up   => app.agent_log_up(),
            _ => {}
        }
        _ => {}
    }
}

// ── Shims tab ─────────────────────────────────────────────────────────────────

fn handle_shims(code: KeyCode, app: &mut App) {
    match code {
        KeyCode::Char('j') | KeyCode::Down => {
            if app.shim_idx + 1 < app.shim_statuses.len() { app.shim_idx += 1; }
        }
        KeyCode::Char('k') | KeyCode::Up => {
            if app.shim_idx > 0 { app.shim_idx -= 1; }
        }
        KeyCode::Char('i') => {
            app.popup = Popup::Confirm {
                message: "Install shims for all known agents?".into(),
                on_yes: ConfirmAction::InstallShims,
            };
        }
        KeyCode::Char('u') => {
            app.popup = Popup::Confirm {
                message: "Uninstall all matis-mem shims?".into(),
                on_yes: ConfirmAction::UninstallShims,
            };
        }
        KeyCode::Char('r') => {
            app.reload_shim_status();
            app.set_status("Refreshed shim status", false);
        }
        _ => {}
    }
}

// ── Knowledge tab ─────────────────────────────────────────────────────────────

fn handle_knowledge(code: KeyCode, mods: KeyModifiers, app: &mut App) {
    match app.focus {
        Focus::KnowledgeList => match code {
            KeyCode::Char('j') | KeyCode::Down => app.knowledge_down(),
            KeyCode::Char('k') | KeyCode::Up   => app.knowledge_up(),
            _ => {}
        }
        _ => {}
    }
    // Ctrl+K from knowledge tab also opens add popup
    if mods.contains(KeyModifiers::CONTROL) && code == KeyCode::Char('k') {
        app.popup = Popup::AddKnowledge {
            topic_buf: String::new(), note_buf: String::new(),
        };
    }
}
