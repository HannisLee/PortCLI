use std::collections::HashMap;
use std::sync::Arc;

use anyhow::{Context, Result};
use serde_json::json;
use tokio::net::TcpListener;
use tokio::sync::{mpsc, watch, Mutex};
use tokio_util::sync::CancellationToken;

use crate::config::{self, Rule};
use crate::control::{self, ControlCommand};
use crate::forward::{self, RuleStatus};
use crate::logs;
use crate::state::{self, RuntimeState};

pub fn run_daemon(foreground: bool) -> Result<()> {
    if foreground {
        let rt = tokio::runtime::Runtime::new()?;
        rt.block_on(async { run_daemon_async().await })
    } else {
        spawn_background_daemon()
    }
}

fn spawn_background_daemon() -> Result<()> {
    if state::is_daemon_running() {
        let state = state::load_state()?;
        println!("daemon is already running (pid: {})", state.pid);
        return Ok(());
    }

    let exe = std::env::current_exe()?;
    let mut cmd = std::process::Command::new(exe);
    cmd.arg("run").arg("--foreground");
    cmd.stdin(std::process::Stdio::null());
    cmd.stdout(std::process::Stdio::null());
    cmd.stderr(std::process::Stdio::null());

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const DETACHED_PROCESS: u32 = 0x00000008;
        cmd.creation_flags(DETACHED_PROCESS);
    }

    let child = cmd.spawn().context("failed to start daemon process")?;
    println!("daemon started (pid: {})", child.id());
    Ok(())
}

fn generate_token() -> String {
    use rand::Rng;
    let token: String = rand::thread_rng()
        .sample_iter(&rand::distributions::Alphanumeric)
        .take(32)
        .map(char::from)
        .collect();
    token
}

struct ManagedRule {
    cancel_token: CancellationToken,
    join_handle: tokio::task::JoinHandle<()>,
    status_rx: watch::Receiver<RuleStatus>,
    rule: Rule,
}

async fn run_daemon_async() -> Result<()> {
    tracing_subscriber::fmt()
        .with_target(false)
        .with_level(true)
        .init();

    let config = config::load_config()?;
    let control_listener = TcpListener::bind("127.0.0.1:0").await?;
    let control_addr = control_listener.local_addr()?;
    let token = generate_token();

    let state = RuntimeState {
        pid: std::process::id(),
        control_host: "127.0.0.1".to_string(),
        control_port: control_addr.port(),
        token: token.clone(),
    };
    state::save_state(&state)?;

    logs::ensure_log_dir()?;
    logs::append_log(
        &logs::get_daemon_log_path()?,
        "INFO",
        &format!("daemon started pid={}", state.pid),
    )?;

    let root_cancel = CancellationToken::new();
    let rules: Arc<Mutex<HashMap<String, ManagedRule>>> = Arc::new(Mutex::new(HashMap::new()));
    let config_arc = Arc::new(Mutex::new(config.clone()));

    let (cmd_tx, mut cmd_rx) = mpsc::channel::<ControlCommand>(32);

    let control_cancel = root_cancel.child_token();
    let control_token = token.clone();
    let control_tx = cmd_tx.clone();
    tokio::spawn(async move {
        if let Err(e) =
            control::run_control_server(control_listener, control_token, control_tx, control_cancel)
                .await
        {
            tracing::error!("control server error: {}", e);
        }
    });

    for rule in &config.rules {
        if rule.enabled {
            start_rule_task(&rules, rule.clone(), &root_cancel).await;
        }
    }

    loop {
        tokio::select! {
            _ = root_cancel.cancelled() => {
                logs::append_log(
                    &logs::get_daemon_log_path()?,
                    "INFO",
                    "daemon stopping (control)",
                )?;
                break;
            }
            _ = tokio::signal::ctrl_c() => {
                logs::append_log(
                    &logs::get_daemon_log_path()?,
                    "INFO",
                    "daemon stopping (signal)",
                )?;
                root_cancel.cancel();
                break;
            }
            Some(cmd) = cmd_rx.recv() => {
                handle_control_command(
                    cmd,
                    &rules,
                    &config_arc,
                    &root_cancel,
                )
                .await;
            }
        }
    }

    root_cancel.cancel();

    let mut rule_map = rules.lock().await;
    for (_, managed) in rule_map.drain() {
        managed.cancel_token.cancel();
        let _ = managed.join_handle.await;
    }
    drop(rule_map);

    state::delete_state()?;
    logs::append_log(&logs::get_daemon_log_path()?, "INFO", "daemon stopped")?;

    Ok(())
}

async fn start_rule_task(
    rules: &Arc<Mutex<HashMap<String, ManagedRule>>>,
    rule: Rule,
    root_cancel: &CancellationToken,
) {
    let name = rule.name.clone();
    let cancel_token = root_cancel.child_token();
    let cancel_for_task = cancel_token.clone();
    let (status_tx, status_rx) = watch::channel(RuleStatus::Starting);
    let rule_clone = rule.clone();

    let join_handle = tokio::spawn(async move {
        forward::run_forward(rule_clone, cancel_for_task, status_tx).await;
    });

    let managed = ManagedRule {
        cancel_token,
        join_handle,
        status_rx,
        rule,
    };

    rules.lock().await.insert(name, managed);
}

async fn stop_all_rules(rules: &Arc<Mutex<HashMap<String, ManagedRule>>>) {
    let mut map = rules.lock().await;
    let names: Vec<String> = map.keys().cloned().collect();
    for name in names {
        if let Some(managed) = map.remove(&name) {
            managed.cancel_token.cancel();
            let _ = managed.join_handle.await;
        }
    }
}

async fn handle_control_command(
    cmd: ControlCommand,
    rules: &Arc<Mutex<HashMap<String, ManagedRule>>>,
    config: &Arc<Mutex<config::Config>>,
    root_cancel: &CancellationToken,
) {
    match cmd.command.as_str() {
        "status" => {
            let response = build_status_response(rules).await;
            let _ = cmd.response.send(response);
        }
        "stop" => {
            let resp = json!({"ok": true, "message": "daemon stopping"});
            let _ = cmd.response.send(resp);
            root_cancel.cancel();
        }
        "reload" => match config::load_config() {
            Ok(new_config) => {
                stop_all_rules(rules).await;
                *config.lock().await = new_config.clone();
                for rule in &new_config.rules {
                    if rule.enabled {
                        start_rule_task(rules, rule.clone(), root_cancel).await;
                    }
                }
                let _ = logs::append_log(
                    &logs::get_daemon_log_path().unwrap_or_default(),
                    "INFO",
                    "config reloaded",
                );
                let resp = json!({"ok": true, "message": "config reloaded"});
                let _ = cmd.response.send(resp);
            }
            Err(e) => {
                let resp = json!({"ok": false, "error": format!("failed to reload config: {}", e)});
                let _ = cmd.response.send(resp);
            }
        },
        _ => {
            let resp = json!({"ok": false, "error": format!("unknown command: {}", cmd.command)});
            let _ = cmd.response.send(resp);
        }
    }
}

async fn build_status_response(
    rules: &Arc<Mutex<HashMap<String, ManagedRule>>>,
) -> serde_json::Value {
    let map = rules.lock().await;
    let mut rules_status = Vec::new();

    for (_, managed) in map.iter() {
        let status = managed.status_rx.borrow().clone();
        let (status_str, error) = match status {
            RuleStatus::Starting => ("starting".to_string(), None),
            RuleStatus::Running => ("running".to_string(), None),
            RuleStatus::Stopped => ("stopped".to_string(), None),
            RuleStatus::Failed(e) => ("failed".to_string(), Some(e)),
        };

        rules_status.push(json!({
            "name": managed.rule.name,
            "source": managed.rule.source,
            "target": managed.rule.target,
            "enabled": managed.rule.enabled,
            "status": status_str,
            "error": error,
        }));
    }

    json!({
        "ok": true,
        "pid": std::process::id(),
        "control": format!("127.0.0.1:{}", {
            state::load_state().map(|s| s.control_port).unwrap_or(0)
        }),
        "rules": rules_status,
    })
}
