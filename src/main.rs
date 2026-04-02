#![allow(dead_code, unreachable_patterns)]
mod config;
mod context;
mod data;
mod error;
mod executor;
mod watcher;
mod ui;

use anyhow::Result;
use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io;
use std::time::{Duration, Instant};
use ui::app::App;

fn main() {
    for arg in std::env::args().skip(1) {
        match arg.as_str() {
            "--version" | "-V" => { println!("matis-mem v{}", env!("CARGO_PKG_VERSION")); return; }
            "--help" | "-h"    => { print_help(); return; }
            _ => {}
        }
    }
    if !is_tty() {
        eprintln!("matis-mem: requires an interactive terminal");
        std::process::exit(1);
    }
    if let Err(e) = run() {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen, DisableMouseCapture);
        eprintln!("matis-mem: {}", e);
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    config::init();
    config::ensure_dirs()?;

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    terminal.clear()?;

    let result = run_app(&mut terminal);

    let _ = disable_raw_mode();
    let _ = execute!(terminal.backend_mut(), LeaveAlternateScreen, DisableMouseCapture);
    let _ = terminal.show_cursor();
    result
}

fn run_app(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> Result<()> {
    let mut app = App::new()?;
    let tick = Duration::from_millis(100);
    let mut last = Instant::now();

    loop {
        terminal.draw(|f| ui::render::render(f, &app))?;

        let timeout = tick.saturating_sub(last.elapsed());
        if crossterm::event::poll(timeout)? {
            let ev = crossterm::event::read()?;
            ui::events::handle(&ev, &mut app);
        }

        if last.elapsed() >= tick {
            app.tick();
            last = Instant::now();
        }

        if app.should_quit { break; }
    }
    Ok(())
}

fn is_tty() -> bool {
    use std::os::unix::io::AsRawFd;
    unsafe { libc::isatty(io::stdout().as_raw_fd()) != 0 }
}

fn print_help() {
    println!("matis-mem v{}", env!("CARGO_PKG_VERSION"));
    println!();
    println!("USAGE");
    println!("  matis-mem              Launch TUI");
    println!("  matis-mem --version    Version");
    println!("  matis-mem --help       Help");
    println!();
    println!("DATA       ~/.matis-mem/");
    println!("SHIMS      ~/.matis-mem/shims/   (install via [3] SHIMS tab)");
    println!();
    println!("TABS");
    println!("  [1] RUN        Run prompts against any model with memory context");
    println!("  [2] AGENTS     Live feed of external agent sessions (claude, amp, etc.)");
    println!("  [3] SHIMS      Install/manage logging wrappers for agent CLIs");
    println!("  [4] KNOWLEDGE  Browse and add your knowledge base");
    println!();
    println!("MODELS SUPPORTED");
    println!("  ollama/llama3     ollama/mistral    ollama/codellama");
    println!("  gemini-cli        claude --print    claude code");
    println!("  amp               vibe              mistral CLI");
    println!();
    println!("SHIM AGENTS");
    println!("  claude  amp  gemini  vibe  aider  copilot  mistral  ollama");
    println!("  All calls from ANY terminal are auto-logged when shims are active.");
}
