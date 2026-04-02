use anyhow::Result;
use notify::{Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;

use crate::config::external_dir;
use crate::data::AgentLog;

/// Message sent from the background watcher thread to the TUI
pub enum WatchEvent {
    /// A new agent log file was created and parsed successfully
    NewLog(AgentLog),
    /// Watcher error (non-fatal)
    Error(String),
}

/// Spawns a background thread watching `~/.matis-mem/external/` for new JSON files.
/// Returns a Receiver you poll each tick.
pub fn start() -> Result<Receiver<WatchEvent>> {
    let (tx, rx) = mpsc::channel::<WatchEvent>();
    let watch_dir = external_dir();
    std::fs::create_dir_all(&watch_dir)?;

    thread::spawn(move || {
        if let Err(e) = watch_loop(watch_dir, tx.clone()) {
            let _ = tx.send(WatchEvent::Error(e.to_string()));
        }
    });

    Ok(rx)
}

fn watch_loop(watch_dir: PathBuf, tx: Sender<WatchEvent>) -> Result<()> {
    let (fs_tx, fs_rx) = mpsc::channel::<notify::Result<Event>>();

    let mut watcher = RecommendedWatcher::new(fs_tx, Config::default())?;
    watcher.watch(&watch_dir, RecursiveMode::Recursive)?;

    for res in fs_rx {
        match res {
            Ok(event) => {
                // Only care about Create events for .json files
                if matches!(event.kind, EventKind::Create(_)) {
                    for path in event.paths {
                        if path.extension().and_then(|x| x.to_str()) == Some("json") {
                            // Small delay to let the shim finish writing
                            thread::sleep(std::time::Duration::from_millis(50));
                            match AgentLog::load(&path) {
                                Ok(log) => { let _ = tx.send(WatchEvent::NewLog(log)); }
                                Err(_)  => {} // file still being written, ignore
                            }
                        }
                    }
                }
            }
            Err(e) => {
                let _ = tx.send(WatchEvent::Error(e.to_string()));
            }
        }
    }
    Ok(())
}
