use anyhow::Result;
use serde_json::json;
use std::io::{BufRead, BufReader, Write};
use std::net::TcpStream;
use std::time::Duration;
use tokio::net::TcpListener;
use tokio::sync::{mpsc, oneshot};
use tokio_util::sync::CancellationToken;

use crate::error::PortCliError;

pub struct ControlCommand {
    pub command: String,
    #[allow(dead_code)]
    pub name: Option<String>,
    pub response: oneshot::Sender<serde_json::Value>,
}

pub async fn run_control_server(
    listener: TcpListener,
    token: String,
    daemon_tx: mpsc::Sender<ControlCommand>,
    cancel_token: CancellationToken,
) -> Result<()> {
    loop {
        tokio::select! {
            _ = cancel_token.cancelled() => {
                return Ok(());
            }
            result = listener.accept() => {
                match result {
                    Ok((stream, _)) => {
                        let t = token.clone();
                        let tx = daemon_tx.clone();
                        tokio::spawn(handle_control_connection(stream, t, tx));
                    }
                    Err(e) => {
                        tracing::error!("control accept error: {}", e);
                    }
                }
            }
        }
    }
}

async fn handle_control_connection(
    mut stream: tokio::net::TcpStream,
    expected_token: String,
    tx: mpsc::Sender<ControlCommand>,
) {
    use tokio::io::AsyncBufReadExt;
    use tokio::io::AsyncWriteExt;

    let mut buf = Vec::new();
    let mut reader = tokio::io::BufReader::new(&mut stream);

    if let Err(e) = reader.read_until(b'\n', &mut buf).await {
        tracing::error!("failed to read control command: {}", e);
        return;
    }

    let cmd: serde_json::Value = match serde_json::from_slice(&buf) {
        Ok(v) => v,
        Err(e) => {
            let resp = json!({"ok": false, "error": format!("invalid json: {}", e)});
            let body = serde_json::to_string(&resp).unwrap() + "\n";
            let _ = stream.write_all(body.as_bytes()).await;
            return;
        }
    };

    let token = cmd.get("token").and_then(|t| t.as_str()).unwrap_or("");
    if token != expected_token {
        let resp = json!({"ok": false, "error": "invalid control token"});
        let body = serde_json::to_string(&resp).unwrap() + "\n";
        let _ = stream.write_all(body.as_bytes()).await;
        return;
    }

    let command = cmd
        .get("command")
        .and_then(|c| c.as_str())
        .unwrap_or("")
        .to_string();
    let name = cmd.get("name").and_then(|n| n.as_str()).map(String::from);

    let (resp_tx, resp_rx) = oneshot::channel();

    if tx
        .send(ControlCommand {
            command,
            name,
            response: resp_tx,
        })
        .await
        .is_err()
    {
        let resp = json!({"ok": false, "error": "daemon not accepting commands"});
        let body = serde_json::to_string(&resp).unwrap() + "\n";
        let _ = stream.write_all(body.as_bytes()).await;
        return;
    }

    match resp_rx.await {
        Ok(response) => {
            let body = serde_json::to_string(&response).unwrap() + "\n";
            let _ = stream.write_all(body.as_bytes()).await;
        }
        Err(_) => {
            let resp = json!({"ok": false, "error": "no response from daemon"});
            let body = serde_json::to_string(&resp).unwrap() + "\n";
            let _ = stream.write_all(body.as_bytes()).await;
        }
    }
}

pub fn send_control_command(command: &str, name: Option<&str>) -> Result<serde_json::Value> {
    let state = crate::state::load_state()?;
    let addr = format!("{}:{}", state.control_host, state.control_port);

    let mut stream =
        TcpStream::connect_timeout(&addr.parse().unwrap(), Duration::from_secs(3))
            .map_err(|e| PortCliError::Control(format!("failed to connect: {}", e)))?;

    let mut cmd = json!({
        "token": state.token,
        "command": command,
    });
    if let Some(n) = name {
        cmd["name"] = serde_json::Value::String(n.to_string());
    }

    let json_str = serde_json::to_string(&cmd)? + "\n";
    stream
        .write_all(json_str.as_bytes())
        .map_err(|e| PortCliError::Control(e.to_string()))?;

    let mut reader = BufReader::new(&stream);
    let mut response = String::new();
    reader
        .read_line(&mut response)
        .map_err(|e| PortCliError::Control(e.to_string()))?;

    let resp: serde_json::Value =
        serde_json::from_str(&response).map_err(|e| PortCliError::Control(e.to_string()))?;
    Ok(resp)
}
