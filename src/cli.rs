use anyhow::Result;
use clap::{Parser, Subcommand};

use crate::config::{self, Protocol, Rule};
use crate::control;
use crate::daemon;
use crate::error::PortCliError;
use crate::logs;
use crate::state;

#[derive(Parser)]
#[command(
    name = "portcli",
    version,
    about = "TCP/UDP port forwarding CLI tool",
    long_about = "portcli manages TCP/UDP port forwarding rules and runs enabled rules through a background daemon. Rules are stored in a TOML config file, while status, stop, reload, and log commands talk to the local daemon.",
    help_template = "{before-help}{name} {version}\n{about-with-newline}\n{usage-heading} {usage}\n\n{all-args}{after-help}",
    after_help = "Examples:\n  portcli --version\n  portcli add web --source 127.0.0.1:9999 --target 127.0.0.1:8080\n  portcli add dns --protocol udp --source 127.0.0.1:5353 --target 1.1.1.1:53\n  portcli enable web\n  portcli run\n  portcli status\n  portcli logs web -n 20\n\nRun 'portcli <COMMAND> --help' for command-specific examples."
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// List all forwarding rules
    #[command(
        long_about = "List all configured forwarding rules. When the daemon is running, portcli also tries to show each rule's live runtime status.",
        after_help = "Examples:\n  portcli list\n  portcli list        # shows configured rules and live status when daemon is running"
    )]
    List,

    /// Add a new forwarding rule
    #[command(
        long_about = "Add a TCP or UDP forwarding rule. The rule name must be unique, and source/target must use host:port format. Newly added rules are disabled by default; run 'portcli enable <name>' before the daemon starts forwarding it. The protocol defaults to tcp.",
        after_help = "Examples:\n  portcli add web --source 127.0.0.1:9999 --target 127.0.0.1:8080\n  portcli add dns --protocol udp --source 127.0.0.1:5353 --target 1.1.1.1:53\n  portcli add lan-web --source 0.0.0.0:8080 --target 127.0.0.1:3000\n  portcli enable web"
    )]
    Add {
        /// Unique rule name
        name: String,
        /// Local listen address in host:port format
        #[arg(long)]
        source: String,
        /// Forward target address in host:port format
        #[arg(long)]
        target: String,
        /// Forwarding protocol
        #[arg(long, value_enum, default_value_t = Protocol::Tcp)]
        protocol: Protocol,
    },

    /// Modify an existing rule
    #[command(
        long_about = "Modify the protocol, source address, target address, or any combination for an existing rule. At least one of --protocol, --source, or --target is required. If the daemon is running, it is notified to reload automatically.",
        after_help = "Examples:\n  portcli modify web --source 0.0.0.0:8081\n  portcli modify web --target 127.0.0.1:5173\n  portcli modify web --protocol udp\n  portcli modify web --source 127.0.0.1:9999 --target 127.0.0.1:8080"
    )]
    Modify {
        /// Existing rule name
        name: String,
        /// New local listen address in host:port format
        #[arg(long)]
        source: Option<String>,
        /// New forward target address in host:port format
        #[arg(long)]
        target: Option<String>,
        /// New forwarding protocol
        #[arg(long, value_enum)]
        protocol: Option<Protocol>,
    },

    /// Remove a rule
    #[command(
        long_about = "Remove a forwarding rule from the config file. If the daemon is running, it is notified to reload and the removed rule stops forwarding.",
        after_help = "Example:\n  portcli remove web"
    )]
    Remove {
        /// Existing rule name
        name: String,
    },

    /// Enable a rule
    #[command(
        long_about = "Enable a configured rule. Enabled rules are started by the daemon. If the daemon is already running, it reloads automatically.",
        after_help = "Examples:\n  portcli enable web\n  portcli status"
    )]
    Enable {
        /// Existing rule name
        name: String,
    },

    /// Disable a rule
    #[command(
        long_about = "Disable a configured rule. Disabled rules remain in the config file but are not started by the daemon. If the daemon is running, it reloads automatically.",
        after_help = "Examples:\n  portcli disable web\n  portcli list"
    )]
    Disable {
        /// Existing rule name
        name: String,
    },

    /// Start the daemon process
    #[command(
        long_about = "Start the forwarding daemon. By default, portcli starts a detached background daemon. Use --foreground when debugging startup, bind errors, or signal handling.",
        after_help = "Examples:\n  portcli run\n  portcli run --foreground"
    )]
    Run {
        /// Run in foreground (for debugging)
        #[arg(long)]
        foreground: bool,
    },

    /// Show daemon and rule status
    #[command(
        long_about = "Show whether the daemon is running, its PID, control address, and live rule statuses. Failed rules include the operating system error message.",
        after_help = "Examples:\n  portcli status\n  portcli status      # useful after enable, disable, modify, or reload"
    )]
    Status,

    /// Stop the daemon
    #[command(
        long_about = "Gracefully stop the daemon. All forwarding tasks are cancelled and the runtime state file is removed.",
        after_help = "Example:\n  portcli stop"
    )]
    Stop,

    /// Reload daemon configuration
    #[command(
        long_about = "Ask a running daemon to reload rules from the config file. CLI commands such as enable, disable, modify, and remove already trigger reload automatically; this command is mainly for manual config edits.",
        after_help = "Examples:\n  portcli reload\n  portcli status"
    )]
    Reload,

    /// View logs
    #[command(
        long_about = "View daemon logs or per-rule logs. Omit the rule name to read the daemon log. Use --follow to keep polling for new lines, --clear to truncate the selected log, or --dir to print the log directory.",
        after_help = "Examples:\n  portcli logs\n  portcli logs web -n 50\n  portcli logs web -f\n  portcli logs --dir\n  portcli logs web --clear"
    )]
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
            protocol,
        } => cmd_add(&name, &source, &target, protocol),
        Commands::Modify {
            name,
            source,
            target,
            protocol,
        } => cmd_modify(&name, source, target, protocol),
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
            "  {:<20} {:<8} {:<24} {:<24} {:<10} {:<12}",
            rule.name, rule.protocol, rule.source, rule.target, rule.enabled, runtime_status
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

fn cmd_add(name: &str, source: &str, target: &str, protocol: Protocol) -> Result<()> {
    config::validate_address(source)?;
    config::validate_address(target)?;

    let mut config = config::load_config()?;

    if config::find_rule(&config, name).is_some() {
        return Err(PortCliError::RuleExists(name.to_string()).into());
    }

    config.rules.push(Rule {
        name: name.to_string(),
        protocol,
        source: source.to_string(),
        target: target.to_string(),
        enabled: false,
    });

    config::save_config(&config)?;
    println!("rule '{}' added", name);
    Ok(())
}

fn cmd_modify(
    name: &str,
    source: Option<String>,
    target: Option<String>,
    protocol: Option<Protocol>,
) -> Result<()> {
    if source.is_none() && target.is_none() && protocol.is_none() {
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
    if let Some(p) = protocol {
        rule.protocol = p;
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
                    "  {:<20} {:<8} {:<24} {:<24} enabled={}",
                    rule.name, rule.protocol, rule.source, rule.target, rule.enabled
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
                        let protocol = rule
                            .get("protocol")
                            .and_then(|p| p.as_str())
                            .unwrap_or("tcp");
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
                        println!("  protocol: {}", protocol);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_defaults_to_tcp() {
        let cli = Cli::try_parse_from([
            "portcli",
            "add",
            "web",
            "--source",
            "127.0.0.1:8080",
            "--target",
            "127.0.0.1:8081",
        ])
        .unwrap();

        match cli.command {
            Commands::Add { protocol, .. } => assert_eq!(protocol, Protocol::Tcp),
            _ => panic!("expected add command"),
        }
    }

    #[test]
    fn add_accepts_udp_protocol() {
        let cli = Cli::try_parse_from([
            "portcli",
            "add",
            "dns",
            "--source",
            "127.0.0.1:5353",
            "--target",
            "1.1.1.1:53",
            "--protocol",
            "udp",
        ])
        .unwrap();

        match cli.command {
            Commands::Add { protocol, .. } => assert_eq!(protocol, Protocol::Udp),
            _ => panic!("expected add command"),
        }
    }

    #[test]
    fn modify_accepts_protocol_only() {
        let cli = Cli::try_parse_from(["portcli", "modify", "dns", "--protocol", "udp"]).unwrap();

        match cli.command {
            Commands::Modify { protocol, .. } => assert_eq!(protocol, Some(Protocol::Udp)),
            _ => panic!("expected modify command"),
        }
    }

    #[test]
    fn protocol_rejects_invalid_value() {
        let result = Cli::try_parse_from([
            "portcli",
            "add",
            "bad",
            "--source",
            "127.0.0.1:1",
            "--target",
            "127.0.0.1:2",
            "--protocol",
            "icmp",
        ]);

        match result {
            Ok(_) => panic!("expected invalid protocol to fail"),
            Err(err) => assert_eq!(err.kind(), clap::error::ErrorKind::InvalidValue),
        }
    }
}
