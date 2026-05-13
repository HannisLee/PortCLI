//! PortHannis 核心模块 - 所有 TCP 转发核心逻辑
//!
//! 本文件包含：
//! - 数据结构定义
//! - TCP 转发核心
//! - 配置管理
//! - 日志轮转
//! - 生命周期管理
//! - HTTP API 处理函数

use std::collections::HashMap;
use std::fs::{self, File, OpenOptions};
use std::io::{BufWriter, Write};
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use axum::{
    extract::{Path as AxumPath, State},
    http::StatusCode,
    Json,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use thiserror::Error;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{mpsc, RwLock};
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use tracing::{debug, info, warn};
use uuid::Uuid;

// === 1. 数据结构 ===

/// 端口转发条目
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForwardingEntry {
    pub id: String,
    pub name: String,
    pub source_address: String,
    pub source_port: u16,
    pub target_address: String,
    pub target_port: u16,
    pub enabled: bool,
    pub log_directory: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// 创建条目请求
#[derive(Debug, Deserialize)]
pub struct EntryRequest {
    pub name: String,
    #[serde(default = "default_source_address")]
    pub source_address: String,
    pub source_port: u16,
    pub target_address: String,
    pub target_port: u16,
    #[serde(default = "default_enabled")]
    pub enabled: bool,
}

fn default_source_address() -> String {
    "0.0.0.0".to_string()
}

fn default_enabled() -> bool {
    true
}

/// CLI modify 命令的部分更新（所有字段可选）
pub struct PartialUpdate {
    pub name: Option<String>,
    pub source_port: Option<u16>,
    pub target_address: Option<String>,
    pub target_port: Option<u16>,
    pub enabled: Option<bool>,
}

/// 条目状态
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum EntryStatus {
    Running,
    Stopped,
    #[serde(rename = "error")]
    Error {
        message: String,
    },
}

/// 日志级别
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LogLevel {
    Info,
    Error,
}

/// 日志事件
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "event")]
pub enum LogEvent {
    #[serde(rename = "connection_accepted")]
    ConnectionAccepted { source: String },
    #[serde(rename = "connection_closed")]
    ConnectionClosed {
        bytes_in: u64,
        bytes_out: u64,
        duration_ms: u64,
    },
    #[serde(rename = "connection_error")]
    ConnectionError { error: String },
    #[serde(rename = "forwarder_started")]
    ForwarderStarted,
    #[serde(rename = "forwarder_stopped")]
    ForwarderStopped,
}

/// 日志消息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogMessage {
    pub timestamp: DateTime<Utc>,
    pub level: LogLevel,
    pub connection_id: String,
    pub event: LogEvent,
}

/// 日志响应
#[derive(Debug, Serialize)]
pub struct LogResponse {
    pub lines: Vec<LogLine>,
    pub total: usize,
    pub offset: usize,
    pub limit: usize,
}

/// 日志行
#[derive(Debug, Serialize, Deserialize)]
pub struct LogLine {
    pub timestamp: String,
    pub level: String,
    pub message: String,
}

/// 配置文件
#[derive(Debug, Serialize, Deserialize)]
struct ConfigFile {
    entries: Vec<ForwardingEntry>,
}

/// 错误类型
#[derive(Debug, Error)]
pub enum CoreError {
    #[error("I/O 错误: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON 错误: {0}")]
    Json(#[from] serde_json::Error),

    #[error("条目未找到: {0}")]
    NotFound(String),

    #[error("端口已被占用: {0}")]
    PortInUse(u16),

    #[error("无效状态: {status} (id: {id})")]
    InvalidState { id: String, status: String },

    #[error("验证错误: {0}")]
    Validation(String),

    #[error("日志轮转错误: {0}")]
    LogRotation(String),
}

// === 2. TCP 转发核心 ===

/// TCP 代理
pub struct TcpProxy {
    entry_id: String,
    source_addr: SocketAddr,
    target_addr: SocketAddr,
    log_tx: mpsc::UnboundedSender<LogMessage>,
    cancel: CancellationToken,
}

impl TcpProxy {
    pub fn new(
        entry_id: String,
        source_addr: SocketAddr,
        target_addr: SocketAddr,
        log_tx: mpsc::UnboundedSender<LogMessage>,
    ) -> Self {
        Self {
            entry_id,
            source_addr,
            target_addr,
            log_tx,
            cancel: CancellationToken::new(),
        }
    }

    pub fn cancellation_token(&self) -> CancellationToken {
        self.cancel.clone()
    }

    pub async fn run(self) -> anyhow::Result<()> {
        let listener = TcpListener::bind(&self.source_addr).await?;

        info!(
            "端口转发启动: {} (entry {}) -> {}",
            self.source_addr, self.entry_id, self.target_addr
        );

        loop {
            tokio::select! {
                result = listener.accept() => {
                    match result {
                        Ok((socket, addr)) => {
                            let entry_id = self.entry_id.clone();
                            let target_addr = self.target_addr;
                            let log_tx = self.log_tx.clone();
                            tokio::spawn(async move {
                                if let Err(e) = Self::handle_connection(socket, addr, entry_id, target_addr, log_tx).await {
                                    tracing::error!("连接处理失败: {}", e);
                                }
                            });
                        }
                        Err(e) => {
                            tracing::error!("接受连接失败: {}", e);
                        }
                    }
                }
                _ = self.cancel.cancelled() => {
                    info!("端口转发停止: {}", self.entry_id);
                    return Ok(());
                }
            }
        }
    }

    async fn handle_connection(
        mut client: TcpStream,
        addr: SocketAddr,
        entry_id: String,
        target_addr: SocketAddr,
        log_tx: mpsc::UnboundedSender<LogMessage>,
    ) -> anyhow::Result<()> {
        let conn_id = Uuid::new_v4().to_string();
        let start_time = std::time::Instant::now();

        log_send(
            &log_tx,
            LogLevel::Info,
            &conn_id,
            LogEvent::ConnectionAccepted {
                source: addr.to_string(),
            },
        );

        match TcpStream::connect(&target_addr).await {
            Ok(mut target) => {
                let client_addr = client.peer_addr()?;
                let target_addr2 = target.peer_addr()?;

                let (mut client_read, mut client_write) = client.split();
                let (mut target_read, mut target_write) = target.split();

                let client_to_target = async {
                    let mut bytes = 0u64;
                    let mut buf = [0u8; 8192];
                    loop {
                        match client_read.read(&mut buf).await {
                            Ok(0) => break,
                            Ok(n) => {
                                bytes += n as u64;
                                if target_write.write_all(&buf[..n]).await.is_err() {
                                    break;
                                }
                            }
                            Err(_) => break,
                        }
                    }
                    bytes
                };

                let target_to_client = async {
                    let mut bytes = 0u64;
                    let mut buf = [0u8; 8192];
                    loop {
                        match target_read.read(&mut buf).await {
                            Ok(0) => break,
                            Ok(n) => {
                                bytes += n as u64;
                                if client_write.write_all(&buf[..n]).await.is_err() {
                                    break;
                                }
                            }
                            Err(_) => break,
                        }
                    }
                    bytes
                };

                let (bytes_in, bytes_out) = tokio::join!(client_to_target, target_to_client);
                let duration_ms = start_time.elapsed().as_millis() as u64;

                drop(client_write);
                drop(target_write);

                log_send(
                    &log_tx,
                    LogLevel::Info,
                    &conn_id,
                    LogEvent::ConnectionClosed {
                        bytes_in,
                        bytes_out,
                        duration_ms,
                    },
                );

                info!(
                    "连接关闭: {} ({}: {} -> {}), bytes: {}/{}",
                    entry_id, conn_id, client_addr, target_addr2, bytes_in, bytes_out
                );
            }
            Err(e) => {
                log_send(
                    &log_tx,
                    LogLevel::Error,
                    &conn_id,
                    LogEvent::ConnectionError {
                        error: e.to_string(),
                    },
                );
            }
        }

        Ok(())
    }
}

fn log_send(
    log_tx: &mpsc::UnboundedSender<LogMessage>,
    level: LogLevel,
    conn_id: &str,
    event: LogEvent,
) {
    let _ = log_tx.send(LogMessage {
        timestamp: Utc::now(),
        level,
        connection_id: conn_id.to_string(),
        event,
    });
}

// === 3. 配置管理 ===

/// 配置存储
pub struct ConfigStore {
    path: PathBuf,
    data: ConfigFile,
}

impl ConfigStore {
    pub fn load() -> Result<Self, CoreError> {
        let exe_dir = std::env::current_exe()
            .map(|p| p.parent().unwrap_or(Path::new(".")).to_path_buf())
            .unwrap_or_else(|_| PathBuf::from("."));
        Self::load_from(exe_dir.join("port.json"))
    }

    pub fn load_from(path: PathBuf) -> Result<Self, CoreError> {
        let data = if path.exists() {
            let content = fs::read_to_string(&path)?;
            serde_json::from_str(&content)?
        } else {
            ConfigFile {
                entries: Vec::new(),
            }
        };
        Ok(Self { path, data })
    }

    pub fn save(&self) -> Result<(), CoreError> {
        let tmp_path = self.path.with_extension("tmp");
        let content = serde_json::to_string_pretty(&self.data)?;
        fs::write(&tmp_path, content)?;
        fs::rename(&tmp_path, &self.path)?;
        Ok(())
    }

    pub fn entries(&self) -> &[ForwardingEntry] {
        &self.data.entries
    }

    pub fn find_entry(&self, id: &str) -> Option<&ForwardingEntry> {
        self.data.entries.iter().find(|e| e.id == id)
    }

    pub fn add_entry(&mut self, req: EntryRequest) -> Result<ForwardingEntry, CoreError> {
        validate_entry_request(&req)?;

        let id = Uuid::new_v4().to_string();
        let now = Utc::now();
        let log_dir = sanitize_dir_name(&req.name);
        let log_dir = format!("logs/{}", log_dir);

        let entry = ForwardingEntry {
            id: id.clone(),
            name: req.name.clone(),
            source_address: req.source_address,
            source_port: req.source_port,
            target_address: req.target_address,
            target_port: req.target_port,
            enabled: req.enabled,
            log_directory: log_dir,
            created_at: now,
            updated_at: now,
        };

        self.data.entries.push(entry.clone());
        self.save()?;
        Ok(entry)
    }

    pub fn update_entry(
        &mut self,
        id: &str,
        req: EntryRequest,
    ) -> Result<ForwardingEntry, CoreError> {
        validate_entry_request(&req)?;

        let entry = self
            .find_entry(id)
            .ok_or_else(|| CoreError::NotFound(id.to_string()))?;

        let index = self.data.entries.iter().position(|e| e.id == id).unwrap();
        let mut updated = entry.clone();
        updated.name = req.name;
        updated.source_address = req.source_address;
        updated.source_port = req.source_port;
        updated.target_address = req.target_address;
        updated.target_port = req.target_port;
        updated.enabled = req.enabled;
        updated.updated_at = Utc::now();

        self.data.entries[index] = updated.clone();
        self.save()?;
        Ok(updated)
    }

    pub fn remove_entry(&mut self, id: &str) -> Result<ForwardingEntry, CoreError> {
        let _entry = self
            .find_entry(id)
            .ok_or_else(|| CoreError::NotFound(id.to_string()))?;
        let index = self.data.entries.iter().position(|e| e.id == id).unwrap();
        Ok(self.data.entries.remove(index))
    }

    pub fn update_entry_partial(
        &mut self,
        id: &str,
        updates: PartialUpdate,
    ) -> Result<ForwardingEntry, CoreError> {
        let entry = self
            .find_entry(id)
            .ok_or_else(|| CoreError::NotFound(id.to_string()))?
            .clone();

        let index = self.data.entries.iter().position(|e| e.id == id).unwrap();
        let mut updated = entry.clone();

        if let Some(name) = updates.name {
            if name.trim().is_empty() {
                return Err(CoreError::Validation("名称不能为空".into()));
            }
            updated.name = name;
        }
        if let Some(source_port) = updates.source_port {
            if source_port == 0 {
                return Err(CoreError::Validation("端口必须在 1-65535 之间".into()));
            }
            updated.source_port = source_port;
        }
        if let Some(target_address) = updates.target_address {
            if target_address.trim().is_empty() {
                return Err(CoreError::Validation("目标地址不能为空".into()));
            }
            updated.target_address = target_address;
        }
        if let Some(target_port) = updates.target_port {
            if target_port == 0 {
                return Err(CoreError::Validation("端口必须在 1-65535 之间".into()));
            }
            updated.target_port = target_port;
        }
        if let Some(enabled) = updates.enabled {
            updated.enabled = enabled;
        }

        updated.updated_at = Utc::now();
        self.data.entries[index] = updated.clone();
        self.save()?;
        Ok(updated)
    }
}

fn validate_entry_request(req: &EntryRequest) -> Result<(), CoreError> {
    if req.name.trim().is_empty() {
        return Err(CoreError::Validation("名称不能为空".into()));
    }
    if req.source_port == 0 || req.target_port == 0 {
        return Err(CoreError::Validation("端口必须在 1-65535 之间".into()));
    }
    if req.target_address.trim().is_empty() {
        return Err(CoreError::Validation("目标地址不能为空".into()));
    }
    Ok(())
}

fn sanitize_dir_name(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' || c == '.' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

// === 4. 日志轮转 ===

/// 条目日志器
pub struct EntryLogger {
    entry_id: String,
    dir: PathBuf,
    writer: Option<BufWriter<File>>,
    current_size: u64,
}

const MAX_SEGMENT_BYTES: u64 = 1_000_000; // 1MB
const MAX_SEGMENTS: usize = 5;

impl EntryLogger {
    pub fn new(dir: PathBuf, entry_id: String) -> Result<Self, CoreError> {
        fs::create_dir_all(&dir)?;

        let current_path = dir.join("current.log");
        let current_size = if current_path.exists() {
            fs::metadata(&current_path)?.len()
        } else {
            0
        };

        let writer = Some(BufWriter::new(
            OpenOptions::new()
                .create(true)
                .append(true)
                .open(&current_path)?,
        ));

        Ok(Self {
            entry_id,
            dir,
            writer,
            current_size,
        })
    }

    pub fn write(&mut self, msg: &LogMessage) -> Result<(), CoreError> {
        let line = format_log_line(msg);
        let bytes = line.as_bytes();

        if self.current_size + bytes.len() as u64 > MAX_SEGMENT_BYTES {
            self.rotate()?;
        }

        if let Some(ref mut writer) = self.writer {
            writer.write_all(bytes)?;
            writer.write_all(b"\n")?;
            writer.flush()?;
            self.current_size += bytes.len() as u64 + 1;
        }

        Ok(())
    }

    fn rotate(&mut self) -> Result<(), CoreError> {
        // 关闭当前文件
        self.writer = None;

        // 删除最旧文件
        let oldest = self.dir.join(format!("current.log.{}", MAX_SEGMENTS));
        if oldest.exists() {
            fs::remove_file(&oldest)?;
        }

        // 重命名文件
        for i in (1..MAX_SEGMENTS).rev() {
            let old_path = self.dir.join(format!("current.log.{}", i));
            let new_path = self.dir.join(format!("current.log.{}", i + 1));
            if old_path.exists() {
                fs::rename(&old_path, &new_path)?;
            }
        }

        // 重命名当前文件
        let current = self.dir.join("current.log");
        let rotated = self.dir.join("current.log.1");
        if current.exists() {
            fs::rename(&current, &rotated)?;
        }

        // 创建新文件
        let new_current = self.dir.join("current.log");
        let writer = Some(BufWriter::new(
            OpenOptions::new()
                .create(true)
                .append(true)
                .open(&new_current)?,
        ));
        self.writer = writer;
        self.current_size = 0;

        Ok(())
    }

    pub fn read_logs(&self, offset: usize, limit: usize) -> Result<LogResponse, CoreError> {
        let mut lines = Vec::new();

        // 读取 current.log
        let current_path = self.dir.join("current.log");
        if current_path.exists() {
            let content = fs::read_to_string(&current_path)?;
            for line in content.lines() {
                if let Ok(parsed) = parse_log_line(line) {
                    lines.push(parsed);
                }
            }
        }

        // 反向读取历史文件
        for i in 1..=MAX_SEGMENTS {
            let log_path = self.dir.join(format!("current.log.{}", i));
            if log_path.exists() {
                let content = fs::read_to_string(&log_path)?;
                for line in content.lines() {
                    if let Ok(parsed) = parse_log_line(line) {
                        lines.push(parsed);
                    }
                }
            }
        }

        lines.reverse();
        let total = lines.len();

        let offset = offset.min(total);
        let limit = limit.min(total - offset);
        let page = lines.into_iter().skip(offset).take(limit).collect();

        Ok(LogResponse {
            lines: page,
            total,
            offset,
            limit,
        })
    }
}

fn format_log_line(msg: &LogMessage) -> String {
    let timestamp = msg.timestamp.format("%Y-%m-%dT%H:%M:%S%.3fZ");
    let level = match msg.level {
        LogLevel::Info => "info",
        LogLevel::Error => "error",
    };

    let event_json = serde_json::to_string(&msg.event).unwrap_or_default();
    let event_str = event_json.trim_start_matches('"').trim_end_matches('"');

    format!("{} [{}] {}", timestamp, level, event_str)
}

fn parse_log_line(line: &str) -> Result<LogLine, CoreError> {
    // 简单解析：YYYY-MM-DDTHH:MM:SS.sssZ [level] message
    let parts: Vec<&str> = line.splitn(3, ' ').collect();
    if parts.len() >= 3 {
        let timestamp = parts[0].to_string();
        let level = parts[1]
            .trim_start_matches('[')
            .trim_end_matches(']')
            .to_string();
        let message = parts[2].to_string();
        Ok(LogLine {
            timestamp,
            level,
            message,
        })
    } else {
        Ok(LogLine {
            timestamp: String::new(),
            level: "info".to_string(),
            message: line.to_string(),
        })
    }
}

// === 5. 生命周期管理 ===

/// 代理句柄
struct ProxyHandle {
    cancel: CancellationToken,
    join_handle: JoinHandle<()>,
}

/// 日志任务句柄
struct LoggerHandle {
    shutdown_tx: mpsc::UnboundedSender<bool>,
    join_handle: JoinHandle<Option<()>>,
}

/// 代理管理器
pub struct ProxyManager {
    config: Arc<RwLock<ConfigStore>>,
    pub proxies: RwLock<HashMap<String, ProxyHandle>>,
    loggers: RwLock<HashMap<String, LoggerHandle>>,
}

impl ProxyManager {
    pub async fn new() -> Result<Self, CoreError> {
        let config = ConfigStore::load()?;
        Ok(Self {
            config: Arc::new(RwLock::new(config)),
            proxies: RwLock::new(HashMap::new()),
            loggers: RwLock::new(HashMap::new()),
        })
    }

    pub async fn list_entries(&self) -> Vec<ForwardingEntry> {
        self.config.read().await.entries().to_vec()
    }

    pub async fn get_entry(&self, id: &str) -> Option<ForwardingEntry> {
        self.config.read().await.find_entry(id).cloned()
    }

    pub async fn create_entry(&self, req: EntryRequest) -> Result<ForwardingEntry, CoreError> {
        self.config.write().await.add_entry(req)
    }

    pub async fn update_entry(
        &self,
        id: &str,
        req: EntryRequest,
    ) -> Result<ForwardingEntry, CoreError> {
        self.config.write().await.update_entry(id, req)
    }

    pub async fn delete_entry(&self, id: &str) -> Result<ForwardingEntry, CoreError> {
        let entry = self.config.write().await.remove_entry(id)?;
        Ok(entry)
    }

    pub async fn start_entry(&self, id: &str) -> Result<EntryStatus, CoreError> {
        // 检查是否已运行
        {
            let proxies = self.proxies.read().await;
            if proxies.contains_key(id) {
                return Ok(EntryStatus::Running);
            }
        }

        let entry = self
            .config
            .read()
            .await
            .find_entry(id)
            .ok_or_else(|| CoreError::NotFound(id.to_string()))?
            .clone();

        let source_addr: SocketAddr = format!("{}:{}", entry.source_address, entry.source_port)
            .parse()
            .map_err(|_| CoreError::Validation("无效的源地址".into()))?;

        let target_addr: SocketAddr = format!("{}:{}", entry.target_address, entry.target_port)
            .parse()
            .map_err(|_| CoreError::Validation("无效的目标地址".into()))?;

        // 创建日志通道
        let (log_tx, log_rx) = mpsc::unbounded_channel();
        let (shutdown_tx, mut shutdown_rx) = mpsc::unbounded_channel();

        // 启动日志任务
        let log_dir = PathBuf::from(&entry.log_directory);
        let entry_id = entry.id.clone();
        let log_handle = tokio::task::spawn_blocking(move || {
            let mut logger = EntryLogger::new(log_dir, entry_id).ok()?;
            while let Some(shutdown) = shutdown_rx.blocking_recv() {
                if shutdown {
                    break;
                }
            }
            Some(())
        });

        // 启动 TCP 代理
        let cancel = CancellationToken::new();
        let proxy = TcpProxy::new(entry.id.clone(), source_addr, target_addr, log_tx.clone());
        let _proxy_cancel = cancel.clone();
        let join_handle = tokio::spawn(async move {
            if let Err(e) = proxy.run().await {
                tracing::error!("代理运行失败 ({}): {}", entry.id, e);
            }
        });

        // 存储句柄
        self.proxies.write().await.insert(
            id.to_string(),
            ProxyHandle {
                cancel,
                join_handle,
            },
        );

        self.loggers.write().await.insert(
            id.to_string(),
            LoggerHandle {
                shutdown_tx,
                join_handle: log_handle,
            },
        );

        Ok(EntryStatus::Running)
    }

    pub async fn stop_entry(&self, id: &str) -> Result<EntryStatus, CoreError> {
        // 取消代理
        let proxy_handle = {
            let mut proxies = self.proxies.write().await;
            proxies.remove(id)
        };

        if let Some(handle) = proxy_handle {
            handle.cancel.cancel();
            let _ =
                tokio::time::timeout(std::time::Duration::from_secs(5), handle.join_handle).await;
        } else {
            return Ok(EntryStatus::Stopped);
        }

        // 关闭日志任务
        if let Some(logger_handle) = self.loggers.write().await.remove(id) {
            let _ = logger_handle.shutdown_tx.send(true);
            let _ =
                tokio::time::timeout(std::time::Duration::from_secs(3), logger_handle.join_handle)
                    .await;
        }

        Ok(EntryStatus::Stopped)
    }

    pub async fn get_status(&self, id: &str) -> Result<EntryStatus, CoreError> {
        self.config
            .read()
            .await
            .find_entry(id)
            .ok_or_else(|| CoreError::NotFound(id.to_string()))?;

        let proxies = self.proxies.read().await;
        if proxies.contains_key(id) {
            Ok(EntryStatus::Running)
        } else {
            Ok(EntryStatus::Stopped)
        }
    }

    pub async fn get_logs(
        &self,
        id: &str,
        offset: usize,
        limit: usize,
    ) -> Result<LogResponse, CoreError> {
        let entry = self
            .config
            .read()
            .await
            .find_entry(id)
            .ok_or_else(|| CoreError::NotFound(id.to_string()))?
            .clone();

        let log_dir = PathBuf::from(&entry.log_directory);
        let logger = EntryLogger::new(log_dir, entry.id)?;
        logger.read_logs(offset, limit)
    }

    pub async fn shutdown_all(&self) {
        let ids: Vec<String> = self.proxies.read().await.keys().cloned().collect();

        for id in ids {
            if let Err(e) = self.stop_entry(&id).await {
                tracing::error!("关闭转发失败 ({}): {}", id, e);
            }
        }
    }

    pub async fn auto_start_enabled(&self) {
        let entries = self.config.read().await.entries().to_vec();
        for entry in entries {
            if entry.enabled {
                match self.start_entry(&entry.id).await {
                    Ok(_) => info!("自动启动: {} ({})", entry.name, entry.id),
                    Err(e) => warn!("自动启动失败 {}: {}", entry.name, e),
                }
            }
        }
    }
}

// === 6. HTTP API 处理函数 ===

/// 应用状态
#[derive(Clone)]
pub struct AppState {
    pub manager: Arc<ProxyManager>,
}

/// 列出所有条目（附带运行状态）
pub async fn list_entries(State(state): State<AppState>) -> Json<Vec<serde_json::Value>> {
    let entries = state.manager.list_entries().await;
    let proxies = state.manager.proxies.read().await;
    let result: Vec<serde_json::Value> = entries
        .into_iter()
        .map(|e| {
            let mut val = serde_json::to_value(&e).unwrap();
            let running = proxies.contains_key(&e.id);
            val.as_object_mut()
                .unwrap()
                .insert("_running".into(), running.into());
            val
        })
        .collect();
    Json(result)
}

/// 创建条目
pub async fn create_entry(
    State(state): State<AppState>,
    Json(req): Json<EntryRequest>,
) -> Result<Json<ForwardingEntry>, CoreError> {
    let entry = state.manager.create_entry(req).await?;
    Ok(Json(entry))
}

/// 获取单个条目
pub async fn get_entry(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<String>,
) -> Result<Json<ForwardingEntry>, CoreError> {
    let entry = state
        .manager
        .get_entry(&id)
        .await
        .ok_or_else(|| CoreError::NotFound(id))?;
    Ok(Json(entry))
}

/// 更新条目
pub async fn update_entry(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<String>,
    Json(req): Json<EntryRequest>,
) -> Result<Json<ForwardingEntry>, CoreError> {
    let entry = state.manager.update_entry(&id, req).await?;
    Ok(Json(entry))
}

/// 删除条目
pub async fn delete_entry(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<String>,
) -> Result<Json<ForwardingEntry>, CoreError> {
    let entry = state.manager.delete_entry(&id).await?;
    Ok(Json(entry))
}

/// 启动条目
pub async fn start_entry(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<String>,
) -> Result<Json<EntryStatus>, CoreError> {
    let status = state.manager.start_entry(&id).await?;
    Ok(Json(status))
}

/// 停止条目
pub async fn stop_entry(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<String>,
) -> Result<Json<EntryStatus>, CoreError> {
    let status = state.manager.stop_entry(&id).await?;
    Ok(Json(status))
}

/// 获取条目状态
pub async fn get_entry_status(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<String>,
) -> Result<Json<EntryStatus>, CoreError> {
    let status = state.manager.get_status(&id).await?;
    Ok(Json(status))
}

/// 获取条目日志
pub async fn get_entry_logs(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<String>,
) -> Result<Json<LogResponse>, CoreError> {
    let logs = state.manager.get_logs(&id, 0, 500).await?;
    Ok(Json(logs))
}

/// 错误响应转换
impl axum::response::IntoResponse for CoreError {
    fn into_response(self) -> axum::http::Response<axum::body::Body> {
        let (status, message) = match &self {
            CoreError::NotFound(_) => (StatusCode::NOT_FOUND, self.to_string()),
            CoreError::InvalidState { .. } => (StatusCode::CONFLICT, self.to_string()),
            CoreError::PortInUse(_) => (StatusCode::CONFLICT, self.to_string()),
            CoreError::Validation(_) => (StatusCode::BAD_REQUEST, self.to_string()),
            _ => (StatusCode::INTERNAL_SERVER_ERROR, "服务器错误".into()),
        };

        let body = Json(serde_json::json!({ "error": message }));
        (status, body).into_response()
    }
}
