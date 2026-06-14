use anyhow::{anyhow, Result};
use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use tokio::net::{TcpListener, UdpSocket};
use tokio::sync::{mpsc, watch};
use tokio_util::sync::CancellationToken;

use crate::config::{Protocol, Rule};
use crate::error::PortCliError;
use crate::logs;

const UDP_BUFFER_SIZE: usize = 65_535;
const UDP_SESSION_QUEUE_CAPACITY: usize = 64;
const UDP_SESSION_IDLE_TIMEOUT: Duration = Duration::from_secs(60);

#[derive(Debug, Clone)]
pub enum RuleStatus {
    Starting,
    Running,
    Stopped,
    Failed(String),
}

struct UdpSession {
    tx: mpsc::Sender<Vec<u8>>,
    cancel_token: CancellationToken,
    join_handle: tokio::task::JoinHandle<()>,
}

#[derive(Clone)]
struct UdpSessionContext {
    target_addr: SocketAddr,
    source_socket: Arc<UdpSocket>,
    done_tx: mpsc::Sender<SocketAddr>,
    parent_cancel: CancellationToken,
    name: String,
    target_label: String,
    log_path: PathBuf,
}

struct UdpSessionTask {
    peer_addr: SocketAddr,
    rx: mpsc::Receiver<Vec<u8>>,
    cancel_token: CancellationToken,
    context: UdpSessionContext,
}

#[derive(Default)]
struct UdpSessionStats {
    packets_sent: u64,
    packets_received: u64,
    bytes_sent: u64,
    bytes_received: u64,
}

pub async fn run_forward(
    rule: Rule,
    cancel_token: CancellationToken,
    status_tx: watch::Sender<RuleStatus>,
) {
    let log_path = logs::get_rule_log_path(&rule.name)
        .unwrap_or_else(|_| std::path::PathBuf::from("/dev/null"));
    let _ = logs::ensure_log_dir();

    let result = match rule.protocol {
        Protocol::Tcp => run_tcp_forward(&rule, &cancel_token, &log_path, &status_tx).await,
        Protocol::Udp => run_udp_forward(&rule, &cancel_token, &log_path, &status_tx).await,
    };

    if let Err(e) = result {
        let _ = logs::append_log(
            &log_path,
            "ERROR",
            &format!(
                "rule failed protocol={} name={} error={}",
                rule.protocol, rule.name, e
            ),
        );
        let _ = status_tx.send(RuleStatus::Failed(e.to_string()));
    }
}

async fn run_tcp_forward(
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
            "rule started protocol=tcp name={} source={} target={}",
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
                    &format!("rule stopping protocol=tcp name={}", rule.name),
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
                            if let Err(e) = handle_tcp_connection(
                                inbound, peer_addr, &target, &name, &log,
                            )
                            .await
                            {
                                let _ = logs::append_log(
                                    &log,
                                    "ERROR",
                                    &format!(
                                        "connection error protocol=tcp name={} peer={} error={}",
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
                            &format!("accept failed protocol=tcp name={} error={}", rule.name, e),
                        );
                    }
                }
            }
        }
    }
}

async fn handle_tcp_connection(
    mut inbound: tokio::net::TcpStream,
    peer_addr: std::net::SocketAddr,
    target: &str,
    name: &str,
    log_path: &Path,
) -> Result<()> {
    let _ = logs::append_log(
        log_path,
        "INFO",
        &format!(
            "connection accepted protocol=tcp name={} peer={}",
            name, peer_addr
        ),
    );

    let mut outbound = tokio::net::TcpStream::connect(target).await.map_err(|e| {
        let _ = logs::append_log(
            log_path,
            "ERROR",
            &format!(
                "connect target failed protocol=tcp name={} target={} error={}",
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
                    "connection closed protocol=tcp name={} peer={} bytes_sent={} bytes_received={}",
                    name, peer_addr, a_to_b, b_to_a
                ),
            );
        }
        Err(e) => {
            let _ = logs::append_log(
                log_path,
                "ERROR",
                &format!(
                    "forward error protocol=tcp name={} peer={} error={}",
                    name, peer_addr, e
                ),
            );
        }
    }

    Ok(())
}

async fn run_udp_forward(
    rule: &Rule,
    cancel_token: &CancellationToken,
    log_path: &Path,
    status_tx: &watch::Sender<RuleStatus>,
) -> Result<()> {
    let source_socket = Arc::new(
        UdpSocket::bind(&rule.source)
            .await
            .map_err(|e| PortCliError::BindFailed(rule.source.clone(), e.to_string()))?,
    );
    let target_addr = resolve_udp_target(&rule.target).await?;

    let _ = logs::append_log(
        log_path,
        "INFO",
        &format!(
            "rule started protocol=udp name={} source={} target={}",
            rule.name, rule.source, rule.target
        ),
    );
    let _ = status_tx.send(RuleStatus::Running);

    let mut buf = vec![0_u8; UDP_BUFFER_SIZE];
    let mut sessions: HashMap<SocketAddr, UdpSession> = HashMap::new();
    let (done_tx, mut done_rx) = mpsc::channel::<SocketAddr>(UDP_SESSION_QUEUE_CAPACITY);
    let session_context = UdpSessionContext {
        target_addr,
        source_socket: Arc::clone(&source_socket),
        done_tx,
        parent_cancel: cancel_token.clone(),
        name: rule.name.clone(),
        target_label: rule.target.clone(),
        log_path: log_path.to_path_buf(),
    };

    loop {
        tokio::select! {
            _ = cancel_token.cancelled() => {
                let _ = logs::append_log(
                    log_path,
                    "INFO",
                    &format!("rule stopping protocol=udp name={}", rule.name),
                );
                let _ = status_tx.send(RuleStatus::Stopped);
                break;
            }
            Some(peer_addr) = done_rx.recv() => {
                sessions.remove(&peer_addr);
            }
            result = source_socket.recv_from(&mut buf) => {
                match result {
                    Ok((len, peer_addr)) => {
                        let packet = buf[..len].to_vec();
                        send_udp_packet_to_session(
                            &mut sessions,
                            peer_addr,
                            packet,
                            &session_context,
                        );
                    }
                    Err(e) => {
                        let _ = logs::append_log(
                            log_path,
                            "ERROR",
                            &format!("recv failed protocol=udp name={} error={}", rule.name, e),
                        );
                    }
                }
            }
        }
    }

    for (_, session) in sessions {
        session.cancel_token.cancel();
        drop(session.tx);
        let _ = session.join_handle.await;
    }

    Ok(())
}

async fn resolve_udp_target(target: &str) -> Result<SocketAddr> {
    let mut addrs = tokio::net::lookup_host(target)
        .await
        .map_err(|e| anyhow!("failed to resolve target {}: {}", target, e))?;
    addrs
        .next()
        .ok_or_else(|| anyhow!("failed to resolve target {}: no addresses", target))
}

fn send_udp_packet_to_session(
    sessions: &mut HashMap<SocketAddr, UdpSession>,
    peer_addr: SocketAddr,
    mut packet: Vec<u8>,
    context: &UdpSessionContext,
) {
    loop {
        let send_result = sessions
            .entry(peer_addr)
            .or_insert_with(|| start_udp_session(peer_addr, context))
            .tx
            .try_send(packet);

        match send_result {
            Ok(()) => return,
            Err(mpsc::error::TrySendError::Full(returned_packet)) => {
                let _ = logs::append_log(
                    &context.log_path,
                    "WARN",
                    &format!(
                        "udp session queue full name={} peer={} target={} dropped_bytes={}",
                        context.name,
                        peer_addr,
                        context.target_label,
                        returned_packet.len()
                    ),
                );
                return;
            }
            Err(mpsc::error::TrySendError::Closed(returned_packet)) => {
                sessions.remove(&peer_addr);
                packet = returned_packet;
            }
        }
    }
}

fn start_udp_session(peer_addr: SocketAddr, context: &UdpSessionContext) -> UdpSession {
    let (tx, rx) = mpsc::channel::<Vec<u8>>(UDP_SESSION_QUEUE_CAPACITY);
    let cancel_token = context.parent_cancel.child_token();
    let cancel_for_task = cancel_token.clone();
    let task = UdpSessionTask {
        peer_addr,
        rx,
        cancel_token: cancel_for_task,
        context: context.clone(),
    };

    let join_handle = tokio::spawn(async move {
        run_udp_session(task).await;
    });

    UdpSession {
        tx,
        cancel_token,
        join_handle,
    }
}

async fn run_udp_session(mut task: UdpSessionTask) {
    let mut stats = UdpSessionStats::default();
    let outbound = match UdpSocket::bind(any_addr_for(task.context.target_addr)).await {
        Ok(socket) => socket,
        Err(e) => {
            let _ = logs::append_log(
                &task.context.log_path,
                "ERROR",
                &format!(
                    "udp session bind failed name={} peer={} target={} error={}",
                    task.context.name, task.peer_addr, task.context.target_label, e
                ),
            );
            let _ = task.context.done_tx.send(task.peer_addr).await;
            return;
        }
    };

    if let Err(e) = outbound.connect(task.context.target_addr).await {
        let _ = logs::append_log(
            &task.context.log_path,
            "ERROR",
            &format!(
                "udp session connect target failed name={} peer={} target={} error={}",
                task.context.name, task.peer_addr, task.context.target_label, e
            ),
        );
        let _ = task.context.done_tx.send(task.peer_addr).await;
        return;
    }

    let _ = logs::append_log(
        &task.context.log_path,
        "INFO",
        &format!(
            "udp session opened name={} peer={} target={}",
            task.context.name, task.peer_addr, task.context.target_label
        ),
    );

    let mut buf = vec![0_u8; UDP_BUFFER_SIZE];

    let close_reason = loop {
        tokio::select! {
            _ = task.cancel_token.cancelled() => {
                break "stopped";
            }
            _ = tokio::time::sleep(udp_session_idle_timeout()) => {
                break "idle_timeout";
            }
            packet = task.rx.recv() => {
                match packet {
                    Some(packet) => {
                        match outbound.send(&packet).await {
                            Ok(n) => {
                                stats.packets_sent += 1;
                                stats.bytes_sent += n as u64;
                            }
                            Err(e) => {
                                let _ = logs::append_log(
                                    &task.context.log_path,
                                    "ERROR",
                                    &format!(
                                        "udp send target failed name={} peer={} target={} error={}",
                                        task.context.name, task.peer_addr, task.context.target_label, e
                                    ),
                                );
                            }
                        }
                    }
                    None => {
                        break "input_closed";
                    }
                }
            }
            result = outbound.recv(&mut buf) => {
                match result {
                    Ok(n) => {
                        match task.context.source_socket.send_to(&buf[..n], task.peer_addr).await {
                            Ok(sent) => {
                                stats.packets_received += 1;
                                stats.bytes_received += sent as u64;
                            }
                            Err(e) => {
                                let _ = logs::append_log(
                                    &task.context.log_path,
                                    "ERROR",
                                    &format!(
                                        "udp send peer failed name={} peer={} target={} error={}",
                                        task.context.name, task.peer_addr, task.context.target_label, e
                                    ),
                                );
                            }
                        }
                    }
                    Err(e) => {
                        let _ = logs::append_log(
                            &task.context.log_path,
                            "ERROR",
                            &format!(
                                "udp recv target failed name={} peer={} target={} error={}",
                                task.context.name, task.peer_addr, task.context.target_label, e
                            ),
                        );
                    }
                }
            }
        }
    };

    let _ = logs::append_log(
        &task.context.log_path,
        "INFO",
        &format!(
            "udp session closed name={} peer={} reason={} packets_sent={} packets_received={} bytes_sent={} bytes_received={}",
            task.context.name,
            task.peer_addr,
            close_reason,
            stats.packets_sent,
            stats.packets_received,
            stats.bytes_sent,
            stats.bytes_received
        ),
    );
    let _ = task.context.done_tx.send(task.peer_addr).await;
}

fn any_addr_for(target_addr: SocketAddr) -> SocketAddr {
    let ip = if target_addr.is_ipv4() {
        IpAddr::V4(Ipv4Addr::UNSPECIFIED)
    } else {
        IpAddr::V6(Ipv6Addr::UNSPECIFIED)
    };
    SocketAddr::new(ip, 0)
}

fn udp_session_idle_timeout() -> Duration {
    std::env::var("PORTCLI_UDP_SESSION_IDLE_TIMEOUT_MS")
        .ok()
        .and_then(|raw| raw.parse::<u64>().ok())
        .filter(|millis| *millis > 0)
        .map(Duration::from_millis)
        .unwrap_or(UDP_SESSION_IDLE_TIMEOUT)
}
