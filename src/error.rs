use thiserror::Error;

#[derive(Error, Debug)]
#[allow(dead_code)]
pub enum PortCliError {
    #[error("rule '{0}' already exists")]
    RuleExists(String),

    #[error("rule '{0}' not found")]
    RuleNotFound(String),

    #[error("invalid address '{0}': {1}")]
    InvalidAddress(String, String),

    #[error("no changes specified")]
    NoChanges,

    #[error("daemon is already running (pid: {0})")]
    DaemonAlreadyRunning(u32),

    #[error("daemon is not running")]
    DaemonNotRunning,

    #[error("invalid control token")]
    InvalidToken,

    #[error("log file not found: {0}")]
    LogFileNotFound(String),

    #[error("failed to read config: {0}")]
    ConfigRead(String),

    #[error("failed to write config: {0}")]
    ConfigWrite(String),

    #[error("failed to write runtime state: {0}")]
    StateWrite(String),

    #[error("failed to read runtime state: {0}")]
    StateRead(String),

    #[error("failed to bind {0}: {1}")]
    BindFailed(String, String),

    #[error("control error: {0}")]
    Control(String),

    #[error(transparent)]
    Io(#[from] std::io::Error),
}
