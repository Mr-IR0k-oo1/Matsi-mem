use std::path::PathBuf;
use std::sync::OnceLock;

static DATA_DIR: OnceLock<PathBuf> = OnceLock::new();

pub fn init() {
    let dir = dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".matis-mem");
    DATA_DIR.set(dir).ok();
}

pub fn data_dir()     -> &'static PathBuf { DATA_DIR.get().expect("config::init not called") }
pub fn projects_dir() -> PathBuf { data_dir().join("projects") }
pub fn sessions_dir() -> PathBuf { data_dir().join("sessions") }
pub fn knowledge_dir()-> PathBuf { data_dir().join("knowledge") }
pub fn prompts_dir()  -> PathBuf { data_dir().join("prompts") }
pub fn external_dir() -> PathBuf { data_dir().join("external") }  // agent shim logs
pub fn shims_dir()    -> PathBuf { data_dir().join("shims") }     // generated shim scripts
pub fn state_file()   -> PathBuf { data_dir().join("state.json") }

pub fn ensure_dirs() -> anyhow::Result<()> {
    for d in &[
        projects_dir(), sessions_dir(), knowledge_dir(),
        prompts_dir(), external_dir(), shims_dir(),
    ] {
        std::fs::create_dir_all(d)?;
    }
    Ok(())
}
