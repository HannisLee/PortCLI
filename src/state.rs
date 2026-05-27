use anyhow::{Context, Result};
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::time::Duration;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeState {
    pub pid: u32,
    pub control_host: String,
    pub control_port: u16,
    pub token: String,
}

fn get_project_dirs() -> Result<ProjectDirs> {
    ProjectDirs::from("", "", "portcli").context("could not determine project directories")
}

pub fn get_state_dir() -> Result<PathBuf> {
    Ok(get_project_dirs()?.data_local_dir().to_path_buf())
}

pub fn get_state_path() -> Result<PathBuf> {
    Ok(get_state_dir()?.join("state.json"))
}

pub fn save_state(state: &RuntimeState) -> Result<()> {
    let dir = get_state_dir()?;
    fs::create_dir_all(&dir)?;
    let path = get_state_path()?;
    let json = serde_json::to_string_pretty(state)
        .map_err(|e| crate::error::PortCliError::StateWrite(e.to_string()))?;
    fs::write(&path, json).map_err(|e| crate::error::PortCliError::StateWrite(e.to_string()))?;
    Ok(())
}

pub fn load_state() -> Result<RuntimeState> {
    let path = get_state_path()?;
    if !path.exists() {
        return Err(
            crate::error::PortCliError::StateRead("state file not found".to_string()).into(),
        );
    }
    let content = fs::read_to_string(&path)
        .map_err(|e| crate::error::PortCliError::StateRead(e.to_string()))?;
    let state: RuntimeState = serde_json::from_str(&content)
        .map_err(|e| crate::error::PortCliError::StateRead(e.to_string()))?;
    Ok(state)
}

pub fn delete_state() -> Result<()> {
    let path = get_state_path()?;
    if path.exists() {
        fs::remove_file(&path)?;
    }
    Ok(())
}

/// Check if daemon is running by trying to connect to its control port.
pub fn is_daemon_running() -> bool {
    match load_state() {
        Ok(s) => {
            let addr = format!("{}:{}", s.control_host, s.control_port);
            std::net::TcpStream::connect_timeout(&addr.parse().unwrap(), Duration::from_millis(500))
                .is_ok()
        }
        Err(_) => false,
    }
}
