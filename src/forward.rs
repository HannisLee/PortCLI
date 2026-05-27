use anyhow::Result;
use std::path::Path;
use tokio::net::TcpListener;
use tokio::sync::watch;
use tokio_util::sync::CancellationToken;

use crate::config::Rule;
use crate::error::PortCliError;
use crate::logs;

#[derive(Debug, Clone)]
pub enum RuleStatus {
    Starting,
    Running,
    Stopped,
    Failed(String),
}

pub async fn run_forward(
    rule: Rule,
    cancel_token: CancellationToken,
    status_tx: watch::Sender<RuleStatus>,
) {
    let log_path = logs::get_rule_log_path(&rule.name)
        .unwrap_or_else(|_| std::path::PathBuf::from("/dev/null"));
    let _ = logs::ensure_log_dir();

    let result = run_forward_inner(&rule, &cancel_token, &log_path, &status_tx).await;

    if let Err(e) = result {
        let _ = logs::append_log(
            &log_path,
            "ERROR",
            &format!("rule failed name={} error={}", rule.name, e),
        );
        let _ = status_tx.send(RuleStatus::Failed(e.to_string()));
    }
}

async fn run_forward_inner(
    rule: &Rule,
    cancel_token: &CancellationToken,
    log_path: &Path,
    status_tx: &watch::Sender<RuleStatus>,
) -> Result<()> {
    let log_path_buf = log_path.to_path_buf();
    let listener = TcpListener::bind(&rule.source)
        .await
        .map_err(|e| PortCliError::BindFailed(rule.source.clone(), e.to_string()))?;

    let _ = logs::append_log(
        log_path,
        "INFO",
        &format!(
            "rule started name={} source={} target={}",
            rule.name, rule.source, rule.target
        ),
    );
    let _ = status_tx.send(RuleStatus::Running);

    loop {
        tokio::select! {
            _ = cancel_token.cancelled() => {
                let _ = logs::append_log(
                    log_path,
                    "INFO",
                    &format!("rule stopping name={}", rule.name),
                );
                let _ = status_tx.send(RuleStatus::Stopped);
                return Ok(());
            }
            result = listener.accept() => {
                match result {
                    Ok((inbound, peer_addr)) => {
                        let target = rule.target.clone();
                        let name = rule.name.clone();
                        let log = log_path_buf.clone();

                        tokio::spawn(async move {
                            if let Err(e) = handle_connection(
                                inbound, peer_addr, &target, &name, &log,
                            )
                            .await
                            {
                                let _ = logs::append_log(
                                    &log,
                                    "ERROR",
                                    &format!(
                                        "connection error name={} peer={} error={}",
                                        name, peer_addr, e
                                    ),
                                );
                            }
                        });
                    }
                    Err(e) => {
                        let _ = logs::append_log(
                            log_path,
                            "ERROR",
                            &format!("accept failed name={} error={}", rule.name, e),
                        );
                    }
                }
            }
        }
    }
}

async fn handle_connection(
    mut inbound: tokio::net::TcpStream,
    peer_addr: std::net::SocketAddr,
    target: &str,
    name: &str,
    log_path: &Path,
) -> Result<()> {
    let _ = logs::append_log(
        log_path,
        "INFO",
        &format!("connection accepted name={} peer={}", name, peer_addr),
    );

    let mut outbound = tokio::net::TcpStream::connect(target).await.map_err(|e| {
        let _ = logs::append_log(
            log_path,
            "ERROR",
            &format!(
                "connect target failed name={} target={} error={}",
                name, target, e
            ),
        );
        e
    })?;

    match tokio::io::copy_bidirectional(&mut inbound, &mut outbound).await {
        Ok((a_to_b, b_to_a)) => {
            let _ = logs::append_log(
                log_path,
                "INFO",
                &format!(
                    "connection closed name={} peer={} bytes_sent={} bytes_received={}",
                    name, peer_addr, a_to_b, b_to_a
                ),
            );
        }
        Err(e) => {
            let _ = logs::append_log(
                log_path,
                "ERROR",
                &format!("forward error name={} peer={} error={}", name, peer_addr, e),
            );
        }
    }

    Ok(())
}
