use anyhow::Result;
use clap::{Parser, Subcommand};

use crate::config::{self, Rule};
use crate::control;
use crate::daemon;
use crate::error::PortCliError;
use crate::logs;
use crate::state;

#[derive(Parser)]
#[command(name = "portcli", version, about = "TCP port forwarding CLI tool")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// List all forwarding rules
    List,

    /// Add a new forwarding rule
    Add {
        name: String,
        #[arg(long)]
        source: String,
        #[arg(long)]
        target: String,
    },

    /// Modify an existing rule
    Modify {
        name: String,
        #[arg(long)]
        source: Option<String>,
        #[arg(long)]
        target: Option<String>,
    },

    /// Remove a rule
    Remove { name: String },

    /// Enable a rule
    Enable { name: String },

    /// Disable a rule
    Disable { name: String },

    /// Start the daemon process
    Run {
        /// Run in foreground (for debugging)
        #[arg(long)]
        foreground: bool,
    },

    /// Show daemon and rule status
    Status,

    /// Stop the daemon
    Stop,

    /// Reload daemon configuration
    Reload,

    /// View logs
    Logs {
        /// Rule name (omit for daemon log)
        name: Option<String>,

        /// Number of lines to show
        #[arg(short = 'n', long = "lines", default_value_t = 100)]
        lines: usize,

        /// Follow log output
        #[arg(short = 'f', long)]
        follow: bool,

        /// Clear the log file
        #[arg(long)]
        clear: bool,

        /// Show log directory path
        #[arg(long)]
        dir: bool,
    },
}

pub fn handle_command(cli: Cli) -> Result<()> {
    match cli.command {
        Commands::List => cmd_list(),
        Commands::Add {
            name,
            source,
            target,
        } => cmd_add(&name, &source, &target),
        Commands::Modify {
            name,
            source,
            target,
        } => cmd_modify(&name, source, target),
        Commands::Remove { name } => cmd_remove(&name),
        Commands::Enable { name } => cmd_enable(&name),
        Commands::Disable { name } => cmd_disable(&name),
        Commands::Run { foreground } => cmd_run(foreground),
        Commands::Status => cmd_status(),
        Commands::Stop => cmd_stop(),
        Commands::Reload => cmd_reload(),
        Commands::Logs {
            name,
            lines,
            follow,
            clear,
            dir,
        } => cmd_logs(name.as_deref(), lines, follow, clear, dir),
    }
}

fn cmd_list() -> Result<()> {
    let config = config::load_config()?;

    if config.rules.is_empty() {
        println!("no rules configured");
        return Ok(());
    }

    let daemon_running = state::is_daemon_running();
    let runtime_rules: Option<serde_json::Value> = if daemon_running {
        match control::send_control_command("status", None) {
            Ok(resp) => resp.get("rules").cloned(),
            Err(_) => None,
        }
    } else {
        None
    };

    for rule in &config.rules {
        let runtime_status = get_rule_runtime_status(&runtime_rules, &rule.name);
        println!(
            "  {:<20} {:<24} {:<24} {:<10} {:<12}",
            rule.name, rule.source, rule.target, rule.enabled, runtime_status
        );
    }

    Ok(())
}

fn get_rule_runtime_status(runtime_rules: &Option<serde_json::Value>, name: &str) -> String {
    match runtime_rules {
        Some(rules) => {
            if let Some(arr) = rules.as_array() {
                for r in arr {
                    if r.get("name").and_then(|n| n.as_str()) == Some(name) {
                        let status = r
                            .get("status")
                            .and_then(|s| s.as_str())
                            .unwrap_or("unknown");
                        return status.to_string();
                    }
                }
            }
            "unknown".to_string()
        }
        None => "unknown".to_string(),
    }
}

fn cmd_add(name: &str, source: &str, target: &str) -> Result<()> {
    config::validate_address(source)?;
    config::validate_address(target)?;

    let mut config = config::load_config()?;

    if config::find_rule(&config, name).is_some() {
        return Err(PortCliError::RuleExists(name.to_string()).into());
    }

    config.rules.push(Rule {
        name: name.to_string(),
        source: source.to_string(),
        target: target.to_string(),
        enabled: false,
    });

    config::save_config(&config)?;
    println!("rule '{}' added", name);
    Ok(())
}

fn cmd_modify(name: &str, source: Option<String>, target: Option<String>) -> Result<()> {
    if source.is_none() && target.is_none() {
        return Err(PortCliError::NoChanges.into());
    }

    if let Some(ref s) = source {
        config::validate_address(s)?;
    }
    if let Some(ref t) = target {
        config::validate_address(t)?;
    }

    let mut config = config::load_config()?;

    let rule = config
        .rules
        .iter_mut()
        .find(|r| r.name == name)
        .ok_or_else(|| PortCliError::RuleNotFound(name.to_string()))?;

    if let Some(s) = source {
        rule.source = s;
    }
    if let Some(t) = target {
        rule.target = t;
    }

    config::save_config(&config)?;
    println!("rule '{}' modified", name);

    notify_daemon_if_running(name);
    Ok(())
}

fn cmd_remove(name: &str) -> Result<()> {
    let mut config = config::load_config()?;

    let idx = config::find_rule_index(&config, name)
        .ok_or_else(|| PortCliError::RuleNotFound(name.to_string()))?;

    config.rules.remove(idx);
    config::save_config(&config)?;
    println!("rule '{}' removed", name);

    notify_daemon_if_running(name);
    Ok(())
}

fn cmd_enable(name: &str) -> Result<()> {
    let mut config = config::load_config()?;

    let rule = config
        .rules
        .iter_mut()
        .find(|r| r.name == name)
        .ok_or_else(|| PortCliError::RuleNotFound(name.to_string()))?;

    rule.enabled = true;
    config::save_config(&config)?;
    println!("rule '{}' enabled", name);

    notify_daemon_if_running(name);
    Ok(())
}

fn cmd_disable(name: &str) -> Result<()> {
    let mut config = config::load_config()?;

    let rule = config
        .rules
        .iter_mut()
        .find(|r| r.name == name)
        .ok_or_else(|| PortCliError::RuleNotFound(name.to_string()))?;

    rule.enabled = false;
    config::save_config(&config)?;
    println!("rule '{}' disabled", name);

    notify_daemon_if_running(name);
    Ok(())
}

fn notify_daemon_if_running(_name: &str) {
    if state::is_daemon_running() {
        match control::send_control_command("reload", None) {
            Ok(resp) => {
                if let Some(msg) = resp.get("message").and_then(|m| m.as_str()) {
                    println!("daemon: {}", msg);
                }
            }
            Err(e) => {
                eprintln!("warning: failed to notify daemon: {}", e);
            }
        }
    }
}

fn cmd_run(foreground: bool) -> Result<()> {
    daemon::run_daemon(foreground)
}

fn cmd_status() -> Result<()> {
    if !state::is_daemon_running() {
        println!("daemon: not running");
        let config = config::load_config()?;
        if config.rules.is_empty() {
            println!("no rules configured");
        } else {
            println!();
            println!("configured rules:");
            for rule in &config.rules {
                println!(
                    "  {:<20} {:<24} {:<24} enabled={}",
                    rule.name, rule.source, rule.target, rule.enabled
                );
            }
        }
        return Ok(());
    }

    match control::send_control_command("status", None) {
        Ok(resp) => {
            if let Some(pid) = resp.get("pid").and_then(|p| p.as_u64()) {
                println!("daemon: running");
                println!("pid: {}", pid);
            }
            if let Some(ctrl) = resp.get("control").and_then(|c| c.as_str()) {
                println!("control: {}", ctrl);
            }

            if let Some(rules) = resp.get("rules").and_then(|r| r.as_array()) {
                if rules.is_empty() {
                    println!();
                    println!("no rules configured");
                } else {
                    println!();
                    println!("rules:");
                    for rule in rules {
                        let name = rule.get("name").and_then(|n| n.as_str()).unwrap_or("");
                        let source = rule.get("source").and_then(|s| s.as_str()).unwrap_or("");
                        let target = rule.get("target").and_then(|t| t.as_str()).unwrap_or("");
                        let enabled = rule
                            .get("enabled")
                            .and_then(|e| e.as_bool())
                            .unwrap_or(false);
                        let status = rule
                            .get("status")
                            .and_then(|s| s.as_str())
                            .unwrap_or("unknown");
                        let error = rule.get("error").and_then(|e| e.as_str());

                        println!();
                        println!("- {}", name);
                        println!("  source: {}", source);
                        println!("  target: {}", target);
                        println!("  enabled: {}", enabled);
                        println!("  status: {}", status);
                        if let Some(e) = error {
                            if !e.is_empty() {
                                println!("  error: {}", e);
                            }
                        }
                    }
                }
            }
        }
        Err(e) => {
            println!("daemon: unreachable ({})", e);
        }
    }

    Ok(())
}

fn cmd_stop() -> Result<()> {
    if !state::is_daemon_running() {
        println!("daemon is not running");
        return Ok(());
    }

    match control::send_control_command("stop", None) {
        Ok(resp) => {
            if let Some(msg) = resp.get("message").and_then(|m| m.as_str()) {
                println!("{}", msg);
            } else if let Some(err) = resp.get("error").and_then(|e| e.as_str()) {
                eprintln!("error: {}", err);
            }
        }
        Err(e) => {
            eprintln!("error: {}", e);
        }
    }

    Ok(())
}

fn cmd_reload() -> Result<()> {
    if !state::is_daemon_running() {
        println!("daemon is not running");
        return Ok(());
    }

    match control::send_control_command("reload", None) {
        Ok(resp) => {
            if let Some(msg) = resp.get("message").and_then(|m| m.as_str()) {
                println!("{}", msg);
            } else if let Some(err) = resp.get("error").and_then(|e| e.as_str()) {
                eprintln!("error: {}", err);
            }
        }
        Err(e) => {
            eprintln!("error: {}", e);
        }
    }

    Ok(())
}

fn cmd_logs(name: Option<&str>, lines: usize, follow: bool, clear: bool, dir: bool) -> Result<()> {
    if dir {
        println!("{}", logs::get_log_dir()?.display());
        return Ok(());
    }

    let log_path = match name {
        Some(n) => logs::get_rule_log_path(n)?,
        None => logs::get_daemon_log_path()?,
    };

    if clear {
        logs::clear_log(&log_path)?;
        let label = name.unwrap_or("daemon");
        println!("{} log cleared", label);
        return Ok(());
    }

    if !log_path.exists() {
        let label = name.unwrap_or("daemon");
        println!("{} log not found: {}", label, log_path.display());
        return Ok(());
    }

    if follow {
        logs::follow_log(&log_path, lines)?;
    } else {
        let log_lines = logs::read_last_lines(&log_path, lines)?;
        for line in log_lines {
            println!("{}", line);
        }
    }

    Ok(())
}
