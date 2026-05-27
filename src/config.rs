use anyhow::{Context, Result};
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

use crate::error::PortCliError;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Rule {
    pub name: String,
    pub source: String,
    pub target: String,
    #[serde(default)]
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub rules: Vec<Rule>,
}

impl Default for Config {
    fn default() -> Self {
        Config { rules: Vec::new() }
    }
}

fn get_project_dirs() -> Result<ProjectDirs> {
    ProjectDirs::from("", "", "portcli")
        .context("could not determine project directories")
}

pub fn get_config_dir() -> Result<PathBuf> {
    Ok(get_project_dirs()?.config_dir().to_path_buf())
}

pub fn get_config_path() -> Result<PathBuf> {
    Ok(get_config_dir()?.join("config.toml"))
}

pub fn load_config() -> Result<Config> {
    let path = get_config_path()?;
    if !path.exists() {
        return Ok(Config::default());
    }
    let content =
        fs::read_to_string(&path).map_err(|e| PortCliError::ConfigRead(e.to_string()))?;
    let config: Config =
        toml::from_str(&content).map_err(|e| PortCliError::ConfigRead(e.to_string()))?;
    Ok(config)
}

pub fn save_config(config: &Config) -> Result<()> {
    let dir = get_config_dir()?;
    fs::create_dir_all(&dir).map_err(|e| PortCliError::ConfigWrite(e.to_string()))?;
    let path = get_config_path()?;
    let content = toml::to_string_pretty(config).map_err(|e| PortCliError::ConfigWrite(e.to_string()))?;
    fs::write(&path, content).map_err(|e| PortCliError::ConfigWrite(e.to_string()))?;
    Ok(())
}

pub fn find_rule<'a>(config: &'a Config, name: &str) -> Option<&'a Rule> {
    config.rules.iter().find(|r| r.name == name)
}

pub fn find_rule_index(config: &Config, name: &str) -> Option<usize> {
    config.rules.iter().position(|r| r.name == name)
}

pub fn validate_address(addr: &str) -> Result<()> {
    let parts: Vec<&str> = addr.rsplitn(2, ':').collect();
    if parts.len() != 2 {
        return Err(PortCliError::InvalidAddress(
            addr.to_string(),
            "expected host:port".to_string(),
        )
        .into());
    }
    let port_str = parts[0];
    port_str
        .parse::<u16>()
        .map_err(|_| PortCliError::InvalidAddress(addr.to_string(), "invalid port".to_string()))?;
    Ok(())
}
