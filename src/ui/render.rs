use ratatui::{
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Clear, List, ListItem, Paragraph, Wrap},
    Frame,
};

use super::app::{App, Focus, Popup, RunState, Tab};
use super::theme::*;

pub fn render(f: &mut Frame, app: &App) {
    let area = f.size();
    f.render_widget(Block::default().style(Style::default().bg(BG)), area);

    let root = Layout::vertical([
        Constraint::Length(2),
        Constraint::Fill(1),
        Constraint::Length(1),
    ]).split(area);

    render_header(f, app, root[0]);

    match app.tab {
        Tab::Run       => render_run(f, app, root[1]),
        Tab::Agents    => render_agents(f, app, root[1]),
        Tab::Shims     => render_shims(f, app, root[1]),
        Tab::Knowledge => render_knowledge(f, app, root[1]),
    }

    render_footer(f, app, root[2]);

    match &app.popup {
        Popup::None => {}
        Popup::NewProject   { .. } => render_new_project(f, app, area),
        Popup::AddKnowledge { .. } => render_add_knowledge(f, app, area),
        Popup::Confirm      { .. } => render_confirm(f, app, area),
        Popup::Output       { .. } => render_output_popup(f, app, area),
    }
}

// ── Header ────────────────────────────────────────────────────────────────────

fn render_header(f: &mut Frame, app: &App, area: Rect) {
    let rows = Layout::vertical([Constraint::Length(1), Constraint::Length(1)]).split(area);

    let proj = app.active_project.as_ref().map(|p| p.name.as_str()).unwrap_or("—");
    let mdl  = app.models.get(app.model_idx).map(|m| m.display_name()).unwrap_or_default();
    let run_span = match &app.run_state {
        RunState::Idle    => Span::styled("◎ idle",     Style::default().fg(DIM)),
        RunState::Running => Span::styled("◌ running…", Style::default().fg(YELLOW)),
        RunState::Done    => Span::styled("◉ done",     ok()),
        RunState::Error(_)=> Span::styled("✗ error",    err()),
    };

    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(" ◆ matis-mem  ", accent()),
            Span::styled("project:", dim()), Span::raw(format!(" {}  ", proj)),
            Span::styled("model:", dim()),   Span::raw(format!(" {}  ", mdl)),
            run_span,
        ])).style(Style::default().bg(BG)),
        rows[0],
    );

    // Tab bar
    let mut spans: Vec<Span> = vec![Span::raw(" ")];
    for t in &[Tab::Run, Tab::Agents, Tab::Shims, Tab::Knowledge] {
        let label = if *t == Tab::Agents && app.unread_count > 0 {
            format!("{} ({})", t.label(), app.unread_count)
        } else {
            t.label().to_string()
        };
        if *t == app.tab {
            spans.push(Span::styled(label, Style::default().fg(ACCENT)
                .add_modifier(Modifier::BOLD | Modifier::UNDERLINED)));
        } else {
            spans.push(Span::styled(label, dim()));
        }
        spans.push(Span::raw("  "));
    }
    f.render_widget(
        Paragraph::new(Line::from(spans)).style(Style::default().bg(BG)),
        rows[1],
    );
}

// ═══════════════════════════════════════════════════════════════════════
// TAB 1 — RUN
// ═══════════════════════════════════════════════════════════════════════

fn render_run(f: &mut Frame, app: &App, area: Rect) {
    let cols = Layout::horizontal([
        Constraint::Length(22),
        Constraint::Fill(1),
    ]).split(area);
    render_projects(f, app, cols[0]);
    render_run_main(f, app, cols[1]);
}

fn render_projects(f: &mut Frame, app: &App, area: Rect) {
    let focused = app.focus == Focus::Projects;
    let items: Vec<ListItem> = app.projects.iter().enumerate().map(|(i, n)| {
        ListItem::new(format!(" {}", n))
            .style(if i == app.project_idx { selected() } else { normal() })
    }).collect();
    let display = if items.is_empty() {
        vec![ListItem::new("  (none) [n] new").style(dim())]
    } else { items };
    let mut st = ratatui::widgets::ListState::default();
    st.select(Some(app.project_idx));
    f.render_stateful_widget(
        List::new(display)
            .block(Block::bordered()
                .title(Span::styled(" PROJECTS ", dim()))
                .border_style(border(focused))
                .border_type(BorderType::Rounded)
                .style(Style::default().bg(SURFACE)))
            .highlight_style(selected()),
        area, &mut st,
    );
}

fn render_run_main(f: &mut Frame, app: &App, area: Rect) {
    let rows = Layout::vertical([
        Constraint::Length(5),
        Constraint::Length(8),
        Constraint::Fill(1),
    ]).split(area);
    render_prompt(f, app, rows[0]);
    render_controls(f, app, rows[1]);
    render_response(f, app, rows[2]);
}

fn render_prompt(f: &mut Frame, app: &App, area: Rect) {
    let focused = app.focus == Focus::Prompt;
    let text = if app.prompt.is_empty() && !focused {
        "enter a prompt…".to_string()
    } else {
        format!("{}▌{}", &app.prompt[..app.cursor], &app.prompt[app.cursor..])
    };
    let style = if app.prompt.is_empty() && !focused { dim() } else { normal() };
    f.render_widget(
        Paragraph::new(text.as_str()).style(style)
            .block(Block::bordered()
                .title(Span::styled(" PROMPT ", dim()))
                .border_style(border(focused))
                .border_type(BorderType::Rounded)
                .style(Style::default().bg(SURFACE)))
            .wrap(Wrap { trim: false }),
        area,
    );
}

fn render_controls(f: &mut Frame, app: &App, area: Rect) {
    let cols = Layout::horizontal([
        Constraint::Percentage(55),
        Constraint::Percentage(45),
    ]).split(area);
    render_context_panel(f, app, cols[0]);
    render_model_panel(f, app, cols[1]);
}

fn cb(on: bool) -> Span<'static> {
    if on { Span::styled("[x] ", ok()) } else { Span::styled("[ ] ", dim()) }
}

fn render_context_panel(f: &mut Frame, app: &App, area: Rect) {
    let focused = app.focus == Focus::Context;
    let sess = match app.ctx_sessions {
        0 => "[ ] recent sessions".to_string(),
        n => format!("[x] last {} session{}", n, if n == 1 { "" } else { "s" }),
    };
    let lines = vec![
        Line::from(vec![cb(app.ctx_project),   Span::raw("project context   "), Span::styled("[space]", dim())]),
        Line::from(vec![Span::styled(&sess, if app.ctx_sessions>0{ok()}else{dim()}), Span::raw("  "), Span::styled("[-/+]",dim())]),
        Line::from(vec![cb(app.ctx_knowledge), Span::raw("knowledge search  "), Span::styled("[k]", dim())]),
        Line::from(vec![Span::raw("  "), Span::styled("Ctrl+R / F5 = RUN", accent())]),
    ];
    f.render_widget(
        Paragraph::new(lines)
            .block(Block::bordered()
                .title(Span::styled(" CONTEXT ", dim()))
                .border_style(border(focused))
                .border_type(BorderType::Rounded)
                .style(Style::default().bg(SURFACE))),
        area,
    );
}

fn render_model_panel(f: &mut Frame, app: &App, area: Rect) {
    let focused = app.focus == Focus::Model;
    let mut last_cat = "";
    let mut items: Vec<ListItem> = Vec::new();
    let mut display_idx = 0usize;
    let mut selected_display = 0usize;
    for (i, m) in app.models.iter().enumerate() {
        let cat = m.category();
        if cat != last_cat {
            items.push(ListItem::new(format!(" ─ {} ", cat)).style(dim()));
            display_idx += 1;
            last_cat = cat;
        }
        if i == app.model_idx { selected_display = display_idx; }
        items.push(
            ListItem::new(format!("  {}", m.display_name()))
                .style(if i == app.model_idx { selected() } else { normal() })
        );
        display_idx += 1;
    }
    let mut st = ratatui::widgets::ListState::default();
    st.select(Some(selected_display));
    f.render_stateful_widget(
        List::new(items)
            .block(Block::bordered()
                .title(Span::styled(" MODEL ", dim()))
                .border_style(border(focused))
                .border_type(BorderType::Rounded)
                .style(Style::default().bg(SURFACE)))
            .highlight_style(selected()),
        area, &mut st,
    );
}

fn render_response(f: &mut Frame, app: &App, area: Rect) {
    let focused = app.focus == Focus::Response;
    let (title, content, style) = match &app.run_state {
        RunState::Idle     => (" RESPONSE ", String::new(), dim()),
        RunState::Running  => (" RESPONSE  ◌ running… ", String::new(), dim()),
        RunState::Done     => (" RESPONSE ", app.response.clone(), normal()),
        RunState::Error(e) => (" RESPONSE  ✗ ", format!("Error: {}", e), err()),
    };
    f.render_widget(
        Paragraph::new(content.as_str()).style(style)
            .block(Block::bordered()
                .title(Span::styled(title, dim()))
                .border_style(border(focused))
                .border_type(BorderType::Rounded)
                .style(Style::default().bg(SURFACE)))
            .wrap(Wrap { trim: false })
            .scroll((app.response_scroll as u16, 0)),
        area,
    );
}

// ═══════════════════════════════════════════════════════════════════════
// TAB 2 — AGENTS
// ═══════════════════════════════════════════════════════════════════════

fn render_agents(f: &mut Frame, app: &App, area: Rect) {
    let cols = Layout::horizontal([
        Constraint::Percentage(38),
        Constraint::Percentage(62),
    ]).split(area);
    render_agent_list(f, app, cols[0]);
    render_agent_detail(f, app, cols[1]);
}

fn agent_color(name: &str) -> Style {
    match name {
        "claude"  => Style::default().fg(ratatui::style::Color::Rgb(100,160,220)),
        "amp"     => Style::default().fg(ratatui::style::Color::Rgb(120,190,120)),
        "gemini"  => Style::default().fg(ratatui::style::Color::Rgb(220,190,70)),
        "vibe"    => Style::default().fg(ratatui::style::Color::Rgb(180,100,220)),
        "ollama"  => Style::default().fg(ratatui::style::Color::Rgb(200,140,80)),
        "mistral" => Style::default().fg(ratatui::style::Color::Rgb(220,80,80)),
        "aider"   => Style::default().fg(ratatui::style::Color::Rgb(80,200,180)),
        _         => Style::default().fg(ratatui::style::Color::Rgb(180,180,180)),
    }
}

fn render_agent_list(f: &mut Frame, app: &App, area: Rect) {
    let focused = app.focus == Focus::AgentList;
    let logs = app.filtered_logs();

    let items: Vec<ListItem> = logs.iter().map(|log| {
        let input_preview: String = if log.input.is_empty() {
            log.args.chars().take(38).collect()
        } else {
            log.input.chars().take(38).collect()
        };
        let proj_tag = if log.project.is_empty() { String::new() }
                       else { format!(" [{}]", log.project) };

        ListItem::new(vec![
            Line::from(vec![
                Span::styled(format!(" {:<10}", log.agent), agent_color(&log.agent).add_modifier(Modifier::BOLD)),
                Span::styled(proj_tag, dim()),
            ]),
            Line::from(Span::styled(format!("  {}", input_preview), dim())),
        ])
    }).collect();

    let display = if items.is_empty() {
        vec![
            ListItem::new("  no logs yet").style(dim()),
            ListItem::new("  → go to [3] SHIMS").style(dim()),
            ListItem::new("  → install shims").style(dim()),
        ]
    } else { items };

    let filter_str = app.agent_filter.as_deref()
        .map(|f| format!(" filter:{} ", f))
        .unwrap_or_default();
    let title = format!(" EXTERNAL AGENTS{} ", filter_str);

    let max = logs.len().saturating_sub(1);
    let mut st = ratatui::widgets::ListState::default();
    st.select(Some(app.agent_log_idx.min(max)));

    f.render_stateful_widget(
        List::new(display)
            .block(Block::bordered()
                .title(Span::styled(title, dim()))
                .border_style(border(focused))
                .border_type(BorderType::Rounded)
                .style(Style::default().bg(SURFACE)))
            .highlight_style(selected()),
        area, &mut st,
    );
}

fn render_agent_detail(f: &mut Frame, app: &App, area: Rect) {
    let focused = app.focus == Focus::AgentDetail;
    let (title, content) = match app.selected_log() {
        None => (" detail ".to_string(), "Select a log entry.".to_string()),
        Some(log) => {
            let mut lines = Vec::new();
            lines.push(format!("Agent:    {}", log.agent));
            lines.push(format!("Project:  {}", if log.project.is_empty() { "—" } else { &log.project }));
            lines.push(format!("CWD:      {}", log.cwd));
            lines.push(format!("Time:     {}", &log.timestamp[..19].replace('T'," ")));
            lines.push(format!("Duration: {}ms  Exit: {}", log.duration_ms, log.exit_code));
            lines.push(format!("Capture:  {}", log.capture));
            if !log.args.is_empty() {
                lines.push(String::new());
                lines.push(format!("Args: {}", log.args));
            }
            if !log.input.is_empty() {
                lines.push(String::new());
                lines.push("── Input ─────────────────────────────".into());
                for l in log.input.lines().take(20) {
                    lines.push(l.to_string());
                }
                if log.input.lines().count() > 20 {
                    lines.push(format!("  … ({} more lines)", log.input.lines().count() - 20));
                }
            }
            if !log.output.is_empty() && log.output != "(interactive)" {
                lines.push(String::new());
                lines.push("── Output ────────────────────────────".into());
                for l in log.output.lines().take(40) {
                    lines.push(l.to_string());
                }
                if log.output.lines().count() > 40 {
                    lines.push(format!("  … ({} more lines)", log.output.lines().count() - 40));
                }
            } else if log.output == "(interactive)" {
                lines.push(String::new());
                lines.push("(interactive — I/O not captured)".into());
            }
            (format!(" {} ", log.agent), lines.join("\n"))
        }
    };
    f.render_widget(
        Paragraph::new(content.as_str()).style(normal())
            .block(Block::bordered()
                .title(Span::styled(title, dim()))
                .border_style(border(focused))
                .border_type(BorderType::Rounded)
                .style(Style::default().bg(SURFACE)))
            .wrap(Wrap { trim: false }),
        area,
    );
}

// ═══════════════════════════════════════════════════════════════════════
// TAB 3 — SHIMS
// ═══════════════════════════════════════════════════════════════════════

fn render_shims(f: &mut Frame, app: &App, area: Rect) {
    let warn_h = if app.shims_need_path { 4u16 } else { 0 };
    let rows = Layout::vertical([
        Constraint::Length(warn_h),
        Constraint::Fill(1),
        Constraint::Length(6),
    ]).split(area);

    if app.shims_need_path {
        let shim_dir = crate::config::shims_dir();
        let line = format!("export PATH=\"{}:$PATH\"", shim_dir.display());
        let msg  = format!(
            "⚠  Shim dir not in PATH — shims are installed but won't intercept yet.\n\
             Add to your ~/.zshrc or ~/.bashrc and restart your shell:\n  {}",
            line
        );
        f.render_widget(
            Paragraph::new(msg.as_str())
                .style(Style::default().fg(YELLOW))
                .block(Block::bordered()
                    .border_style(Style::default().fg(YELLOW))
                    .border_type(BorderType::Rounded)
                    .style(Style::default().bg(SURFACE)))
                .wrap(Wrap { trim: false }),
            rows[0],
        );
    }

    // Shim status table
    let focused = app.focus == Focus::ShimList;
    let items: Vec<ListItem> = app.shim_statuses.iter().enumerate().map(|(i, s)| {
        let (icon, icon_style) = if !s.real_exists {
            ("○ not found  ", dim())
        } else if s.installed && s.active_in_path {
            ("● active      ", ok())
        } else if s.installed {
            ("◑ installed   ", Style::default().fg(YELLOW))
        } else {
            ("◌ not shimmed ", dim())
        };
        let name_style = if i == app.shim_idx {
            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD)
        } else { normal() };
        ListItem::new(Line::from(vec![
            Span::styled(format!("  {}", icon), icon_style),
            Span::styled(format!("{:<12}", s.name), name_style),
            if s.installed && !s.active_in_path {
                Span::styled("  ← add shims dir to PATH", Style::default().fg(YELLOW))
            } else { Span::raw("") },
        ]))
    }).collect();

    let mut st = ratatui::widgets::ListState::default();
    st.select(Some(app.shim_idx));
    f.render_stateful_widget(
        List::new(items)
            .block(Block::bordered()
                .title(Span::styled(
                    " SHIMS   ● active  ◑ installed  ◌ not shimmed  ○ not found ",
                    dim()
                ))
                .border_style(border(focused))
                .border_type(BorderType::Rounded)
                .style(Style::default().bg(SURFACE)))
            .highlight_style(selected()),
        rows[1], &mut st,
    );

    // Help box
    f.render_widget(
        Paragraph::new(
            " [i] install all shims    [u] uninstall all    [r] refresh\n\n\
             Shims wrap agent CLIs so every call to 'claude', 'amp', 'gemini', 'vibe',\n\
             'aider', 'mistral', 'ollama' — from ANY terminal tab — is logged to\n\
             ~/.matis-mem/external/ and streamed live into the AGENTS tab."
        )
        .style(dim())
        .block(Block::bordered()
            .border_style(Style::default().fg(BORDER))
            .border_type(BorderType::Rounded)
            .style(Style::default().bg(SURFACE)))
        .wrap(Wrap { trim: false }),
        rows[2],
    );
}

// ═══════════════════════════════════════════════════════════════════════
// TAB 4 — KNOWLEDGE
// ═══════════════════════════════════════════════════════════════════════

fn render_knowledge(f: &mut Frame, app: &App, area: Rect) {
    let cols = Layout::horizontal([
        Constraint::Percentage(32),
        Constraint::Percentage(68),
    ]).split(area);

    let focused_list   = app.focus == Focus::KnowledgeList;
    let focused_detail = app.focus == Focus::KnowledgeDetail;

    let items: Vec<ListItem> = app.knowledge_topics.iter().enumerate().map(|(i, t)| {
        ListItem::new(format!("  {}", t))
            .style(if i == app.knowledge_idx { selected() } else { normal() })
    }).collect();
    let display = if items.is_empty() {
        vec![ListItem::new("  (empty)  Ctrl+K to add").style(dim())]
    } else { items };

    let mut st = ratatui::widgets::ListState::default();
    st.select(Some(app.knowledge_idx));
    f.render_stateful_widget(
        List::new(display)
            .block(Block::bordered()
                .title(Span::styled(" KNOWLEDGE ", dim()))
                .border_style(border(focused_list))
                .border_type(BorderType::Rounded)
                .style(Style::default().bg(SURFACE)))
            .highlight_style(selected()),
        cols[0], &mut st,
    );

    f.render_widget(
        Paragraph::new(app.knowledge_detail.as_str()).style(normal())
            .block(Block::bordered()
                .title(Span::styled(" NOTES ", dim()))
                .border_style(border(focused_detail))
                .border_type(BorderType::Rounded)
                .style(Style::default().bg(SURFACE)))
            .wrap(Wrap { trim: false }),
        cols[1],
    );
}

// ═══════════════════════════════════════════════════════════════════════
// FOOTER
// ═══════════════════════════════════════════════════════════════════════

fn render_footer(f: &mut Frame, app: &App, area: Rect) {
    let text = if let Some((ref msg, is_err, _)) = app.status {
        Paragraph::new(format!(" {}", msg))
            .style(if is_err { err() } else { ok() })
    } else {
        let hint = match (&app.tab, &app.focus) {
            (Tab::Run, Focus::Projects)  => " [j/k] nav  [n] new  [d] del  [Enter] select  [Tab] →prompt",
            (Tab::Run, Focus::Prompt)    => " type  [Enter/Ctrl+R] run  [Shift+Enter] newline  [Tab] →context",
            (Tab::Run, Focus::Context)   => " [Space] project  [-/+] sessions  [k] knowledge  [Tab] →model",
            (Tab::Run, Focus::Model)     => " [j/k] select  [Tab] →response  [Ctrl+R] run",
            (Tab::Run, Focus::Response)  => " [j/k] scroll  [g/G] top/bot  [c] clear  [Ctrl+R] run again",
            (Tab::Agents, Focus::AgentList)   => " [j/k] nav  [Tab] →detail  [f] filter  [a] all  [r] refresh",
            (Tab::Agents, Focus::AgentDetail) => " [j/k] scroll  [Tab] →list",
            (Tab::Shims,  _)             => " [i] install all  [u] uninstall  [r] refresh  [j/k] nav",
            (Tab::Knowledge, Focus::KnowledgeList)   => " [j/k] nav  [Ctrl+K] add  [Tab] →detail",
            (Tab::Knowledge, Focus::KnowledgeDetail) => " [Tab] →list  [Ctrl+K] add entry",
            _                            => " [1-4] tabs  [Ctrl+N] new project  [Ctrl+K] add knowledge  [q] quit",
        };
        Paragraph::new(hint).style(dim())
    };
    f.render_widget(text.style(Style::default().bg(BG)), area);
}

// ═══════════════════════════════════════════════════════════════════════
// POPUPS
// ═══════════════════════════════════════════════════════════════════════

fn render_new_project(f: &mut Frame, app: &App, area: Rect) {
    let Popup::NewProject { name_buf, goal_buf, field } = &app.popup else { return };
    let p = centered(58, 10, area);
    f.render_widget(Clear, p);
    let block = Block::bordered()
        .title(Span::styled(" New Project ", accent()))
        .border_style(Style::default().fg(FOCUS))
        .border_type(BorderType::Rounded)
        .style(Style::default().bg(SURFACE));
    let inner = block.inner(p);
    f.render_widget(block, p);
    let r = Layout::vertical([
        Constraint::Length(1), Constraint::Length(1), Constraint::Length(1),
        Constraint::Length(1), Constraint::Length(1), Constraint::Length(1),
    ]).split(inner);
    f.render_widget(Paragraph::new(Line::from(vec![
        Span::styled("Name: ", if *field==0{accent()}else{dim()}),
        Span::styled(format!("{}▌", name_buf), normal()),
    ])), r[1]);
    f.render_widget(Paragraph::new(Line::from(vec![
        Span::styled("Goal: ", if *field==1{accent()}else{dim()}),
        Span::styled(format!("{}▌", goal_buf), normal()),
    ])), r[3]);
    f.render_widget(
        Paragraph::new("[Tab] next field  [Enter] create  [Esc] cancel")
            .style(dim()).alignment(Alignment::Center), r[5],
    );
}

fn render_add_knowledge(f: &mut Frame, app: &App, area: Rect) {
    let Popup::AddKnowledge { topic_buf, note_buf } = &app.popup else { return };
    let p = centered(60, 10, area);
    f.render_widget(Clear, p);
    let block = Block::bordered()
        .title(Span::styled(" Add Knowledge ", accent()))
        .border_style(Style::default().fg(FOCUS))
        .border_type(BorderType::Rounded)
        .style(Style::default().bg(SURFACE));
    let inner = block.inner(p);
    f.render_widget(block, p);
    let r = Layout::vertical([
        Constraint::Length(1), Constraint::Length(1), Constraint::Length(1),
        Constraint::Length(1), Constraint::Length(1), Constraint::Length(1),
    ]).split(inner);
    f.render_widget(Paragraph::new(format!("Topic:  {}▌", topic_buf)).style(normal()), r[1]);
    f.render_widget(Paragraph::new(format!("Note:   {}▌", note_buf)).style(normal()), r[3]);
    f.render_widget(
        Paragraph::new("[Enter] save  [Esc] cancel").style(dim()).alignment(Alignment::Center),
        r[5],
    );
}

fn render_confirm(f: &mut Frame, app: &App, area: Rect) {
    let Popup::Confirm { message, .. } = &app.popup else { return };
    let p = centered(52, 7, area);
    f.render_widget(Clear, p);
    let block = Block::bordered()
        .title(Span::styled(" Confirm ", accent()))
        .border_style(Style::default().fg(FOCUS))
        .border_type(BorderType::Rounded)
        .style(Style::default().bg(SURFACE));
    let inner = block.inner(p);
    f.render_widget(block, p);
    let r = Layout::vertical([
        Constraint::Fill(1), Constraint::Length(1), Constraint::Fill(1), Constraint::Length(1),
    ]).split(inner);
    f.render_widget(
        Paragraph::new(message.as_str()).style(normal()).alignment(Alignment::Center), r[1],
    );
    f.render_widget(
        Paragraph::new("[y / Enter] yes  [n / Esc] cancel")
            .style(dim()).alignment(Alignment::Center), r[3],
    );
}

fn render_output_popup(f: &mut Frame, app: &App, area: Rect) {
    let Popup::Output { title, lines, scroll } = &app.popup else { return };
    let p = centered(70, 60, area);
    f.render_widget(Clear, p);
    let block = Block::bordered()
        .title(Span::styled(format!(" {} ", title), accent()))
        .border_style(Style::default().fg(FOCUS))
        .border_type(BorderType::Rounded)
        .style(Style::default().bg(SURFACE));
    let inner = block.inner(p);
    f.render_widget(block, p);

    let h = inner.height.saturating_sub(1) as usize;
    let visible: Vec<ListItem> = lines.iter()
        .skip(*scroll).take(h)
        .map(|l| {
            let s = if l.contains('✓') || l.contains("Done")  { ok() }
                    else if l.contains('✗') || l.contains("Error") { err() }
                    else if l.contains('⚠') { Style::default().fg(YELLOW) }
                    else { normal() };
            ListItem::new(l.as_str()).style(s)
        }).collect();
    f.render_widget(List::new(visible).style(Style::default().bg(SURFACE)), inner);

    let hint_y = inner.y + inner.height.saturating_sub(1);
    f.render_widget(
        Paragraph::new(" [j/k] scroll  any key dismiss").style(dim()),
        Rect { x: inner.x, y: hint_y, width: inner.width, height: 1 },
    );
}

fn centered(pct_x: u16, height: u16, r: Rect) -> Rect {
    let v = Layout::vertical([
        Constraint::Fill(1), Constraint::Length(height), Constraint::Fill(1),
    ]).split(r);
    Layout::horizontal([
        Constraint::Percentage((100 - pct_x) / 2),
        Constraint::Percentage(pct_x),
        Constraint::Percentage((100 - pct_x) / 2),
    ]).split(v[1])[1]
}
