# portcli

A cross-platform TCP port forwarding CLI tool written in Rust. Manage forwarding rules and run them through a background daemon — no root required.

[![Rust](https://img.shields.io/badge/rust-1.70+-orange.svg)](https://www.rust-lang.org)
[![Platform](https://img.shields.io/badge/platform-linux%20%7C%20windows-blue.svg)](#)
[![Tests](https://img.shields.io/badge/tests-105%2F105%20passed-brightgreen.svg)](#)

## Features

- Manage TCP forwarding rules via CLI — add, modify, remove, enable, disable
- Background daemon executes rules persistently
- Local TCP JSON control protocol for daemon management
- Per-rule and daemon-level structured logging
- Cross-platform: Linux (XDG) and Windows (AppData) path conventions
- Zero privileged-port or root requirement

## Installation

Requires Rust 1.70 or later.

```bash
cargo build --release
```

The binary is at `target/release/portcli` (Linux) or `target\release\portcli.exe` (Windows). Add it to your `PATH` for convenience.

## Quick Start

```bash
# Add a rule (disabled by default)
portcli add web --source 0.0.0.0:8080 --target 192.168.31.10:8080

# Enable it
portcli enable web

# Start the daemon in the background
portcli run

# Check what's happening
portcli status
portcli logs web
```

---

## Commands

### `portcli list`

List all rules with name, source, target, enabled flag, and runtime status. If the daemon is running, live status is shown; otherwise `unknown`.

### `portcli add <name> --source <addr> --target <addr>`

Add a new rule. Name must be unique. Addresses must be valid `host:port`. Created with `enabled = false`.

```bash
portcli add web --source 0.0.0.0:8080 --target 192.168.31.10:8080
```

### `portcli modify <name> [--source <addr>] [--target <addr>]`

Modify an existing rule. Only the flags you provide are changed. If the daemon is running, it is notified to reload.

```bash
portcli modify web --source 0.0.0.0:8081
portcli modify web --target 192.168.31.20:8080
```

### `portcli remove <name>`

Delete a rule. If the daemon is running, it reloads and stops forwarding for that rule.

### `portcli enable <name>`

Set `enabled = true`. If the daemon is running, it reloads and starts the rule.

### `portcli disable <name>`

Set `enabled = false`. If the daemon is running, it reloads and stops the rule.

### `portcli run`

Start the daemon in the background. Reads the config and starts every enabled rule.

```bash
portcli run --foreground   # stay in the foreground (useful for debugging)
```

### `portcli status`

Show whether the daemon is running, its PID, control address, and every rule's runtime status. Failed rules include the OS error message.

### `portcli stop`

Gracefully stop the daemon. All forwarding tasks are cancelled, the runtime state file is removed, and the process exits.

### `portcli reload`

Tell a running daemon to reload its configuration from disk. All rules are stopped and restarted based on the current config.

### `portcli logs [name] [-n <lines>] [-f] [--clear] [--dir]`

View and manage log files.

| Usage | Effect |
|-------|--------|
| `portcli logs` | Last 100 lines of daemon log |
| `portcli logs <name>` | Last 100 lines of a rule's log |
| `-n <N>` / `--lines <N>` | Show last N lines (default 100) |
| `-f` / `--follow` | Follow mode — polls every 500 ms, Ctrl+C to exit |
| `--clear` | Truncate the log file |
| `--dir` | Print the log directory path |

---

## File Locations

### Configuration (TOML)

| Platform | Path |
|----------|------|
| Linux | `~/.config/portcli/config.toml` |
| Windows | `%APPDATA%\portcli\config\config.toml` |

### Logs & Runtime State

| Platform | Path |
|----------|------|
| Linux | `~/.local/share/portcli/logs/` |
| Windows | `%LOCALAPPDATA%\portcli\data\logs\` |

```
logs/
├── daemon.log
└── rules/
    ├── web.log
    └── ssh.log
```

The runtime state file (`state.json`) is written to the same parent directory as the logs. It is deleted automatically when the daemon stops.

### Example `config.toml`

```toml
[[rules]]
name = "web"
source = "0.0.0.0:8080"
target = "192.168.31.10:8080"
enabled = true

[[rules]]
name = "ssh"
source = "127.0.0.1:2222"
target = "192.168.31.20:22"
enabled = false
```

---

## Architecture

```
CLI (cli.rs)
 │
 ├── config read/write ──────────► config.toml
 │
 ├── control commands ───────────► Daemon (daemon.rs)
 │    (TCP JSON, 127.0.0.1)          │
 │                                   ├── Control Server (control.rs)
 │                                   ├── Forward Tasks  (forward.rs)
 │                                   └── Log Writer     (logs.rs)
 │
 └── log viewing ────────────────► log files
```

1. **Config**: Rules stored as TOML. CLI and daemon share the same file.
2. **Daemon**: Binds a random localhost control port. Starts a TCP listener per enabled rule.
3. **Forwarding**: Each inbound connection spawns a tokio task. Bidirectional copy via `tokio::io::copy_bidirectional`.
4. **Control**: CLI sends JSON commands authenticated by a random token stored in the state file.
5. **Logging**: Daemon events and per-connection details written to timestamped log files.

---

## Testing a Forwarding Rule

**1. Start a backend listener on the target port:**

```bash
# Linux / WSL
nc -l 8080
```

```powershell
# Windows PowerShell
$l = [System.Net.Sockets.TcpListener]::new([System.Net.IPAddress]::Parse("127.0.0.1"), 8080)
$l.Start(); $c = $l.AcceptTcpClient()
# … read/write $c.GetStream()
```

**2. Add and enable a rule:**

```bash
portcli add test --source 127.0.0.1:9999 --target 127.0.0.1:8080
portcli enable test
```

**3. Start the daemon:**

```bash
portcli run --foreground
```

**4. Connect through the forwarded port:**

```bash
# Linux
echo "hello" | nc 127.0.0.1 9999
```

```powershell
# Windows PowerShell
$c = New-Object System.Net.Sockets.TcpClient("127.0.0.1", 9999)
# … write/read $c.GetStream()
```

**5. Verify in the logs:**

```bash
portcli logs test
# [2026-05-27 12:00:10] INFO connection accepted name=test peer=127.0.0.1:53422
# [2026-05-27 12:00:11] INFO connection closed name=test peer=127.0.0.1:53422 bytes_sent=5 bytes_received=0
```

---

## Project Structure

```
src/
├── main.rs      # Entry point
├── cli.rs       # CLI arg parsing (clap derive) and command handlers
├── config.rs    # TOML config read/write
├── forward.rs   # TCP forwarding (tokio::io::copy_bidirectional)
├── daemon.rs    # Daemon lifecycle, rule management, graceful shutdown
├── control.rs   # TCP JSON control server + client
├── state.rs     # Runtime state file (JSON)
├── logs.rs      # Log utilities: paths, append, read, follow, clear
└── error.rs     # Error types (thiserror)
```

---

## Limitations (MVP)

- TCP only (no UDP)
- No TLS encryption
- No systemd / Windows Service integration
- No GUI
- No config hot-reload (requires manual `portcli reload`)
- No authentication beyond the local control token
- Log follow uses polling (500 ms interval), not native `inotify` / `ReadDirectoryChangesW`

---

## Test Results

| Platform | Tests | Passed |
|----------|-------|--------|
| Windows 11 (MSVC) | 53 | 53 |
| Ubuntu 24.04 / WSL2 (GNU) | 52 | 52 |

Detailed reports: [TEST_REPORT.md](TEST_REPORT.md) · [TEST_REPORT_LINUX.md](TEST_REPORT_LINUX.md)

---

## License

MIT
