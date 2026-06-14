use anyhow::{Context, Result};
use clap::ValueEnum;
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::fs;
use std::path::PathBuf;

use crate::error::PortCliError;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum Protocol {
    #[default]
    Tcp,
    Udp,
}

impl fmt::Display for Protocol {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Protocol::Tcp => write!(f, "tcp"),
            Protocol::Udp => write!(f, "udp"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Rule {
    pub name: String,
    #[serde(default)]
    pub protocol: Protocol,
    pub source: String,
    pub target: String,
    #[serde(default)]
    pub enabled: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub rules: Vec<Rule>,
}

fn get_project_dirs() -> Result<ProjectDirs> {
    ProjectDirs::from("", "", "portcli").context("could not determine project directories")
}

pub fn get_config_dir() -> Result<PathBuf> {
    if let Ok(dir) = std::env::var("PORTCLI_CONFIG_DIR") {
        if !dir.is_empty() {
            return Ok(PathBuf::from(dir));
        }
    }
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
    let content = fs::read_to_string(&path).map_err(|e| PortCliError::ConfigRead(e.to_string()))?;
    let config: Config =
        toml::from_str(&content).map_err(|e| PortCliError::ConfigRead(e.to_string()))?;
    Ok(config)
}

pub fn save_config(config: &Config) -> Result<()> {
    let dir = get_config_dir()?;
    fs::create_dir_all(&dir).map_err(|e| PortCliError::ConfigWrite(e.to_string()))?;
    let path = get_config_path()?;
    let content =
        toml::to_string_pretty(config).map_err(|e| PortCliError::ConfigWrite(e.to_string()))?;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_protocol_defaults_to_tcp() {
        let input = r#"
[[rules]]
name = "web"
source = "127.0.0.1:8080"
target = "127.0.0.1:8081"
enabled = true
"#;

        let config: Config = toml::from_str(input).unwrap();

        assert_eq!(config.rules[0].protocol, Protocol::Tcp);
    }

    #[test]
    fn protocol_serializes_as_lowercase() {
        let config = Config {
            rules: vec![
                Rule {
                    name: "tcp_rule".to_string(),
                    protocol: Protocol::Tcp,
                    source: "127.0.0.1:1000".to_string(),
                    target: "127.0.0.1:1001".to_string(),
                    enabled: false,
                },
                Rule {
                    name: "udp_rule".to_string(),
                    protocol: Protocol::Udp,
                    source: "127.0.0.1:2000".to_string(),
                    target: "127.0.0.1:2001".to_string(),
                    enabled: true,
                },
            ],
        };

        let output = toml::to_string_pretty(&config).unwrap();
        assert!(output.contains("protocol = \"tcp\""));
        assert!(output.contains("protocol = \"udp\""));

        let parsed: Config = toml::from_str(&output).unwrap();
        assert_eq!(parsed.rules[0].protocol, Protocol::Tcp);
        assert_eq!(parsed.rules[1].protocol, Protocol::Udp);
    }
}
