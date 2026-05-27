# Changelog

All notable changes to PortHannis / portcli are documented in this file.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

---

## [0.4.0] — 2026-05-27

### Rewritten

portcli: a complete ground-up rewrite as a pure CLI TCP port forwarding tool. No GUI, no WebUI, no Tauri — just a focused CLI daemon.

### Added

- **11 CLI commands** via `clap` derive:
  - `list` — show all rules with runtime status
  - `add` — create a rule (`--source`, `--target`)
  - `modify` — update source/target of an existing rule
  - `remove` — delete a rule
  - `enable` / `disable` — toggle rule on/off
  - `run` / `run --foreground` — start daemon (background or foreground)
  - `status` — daemon PID, control address, per-rule status
  - `stop` — graceful shutdown
  - `reload` — re-read config and restart rules
  - `logs` — view/follow/clear daemon and per-rule logs
- **Background daemon** with TCP JSON control protocol on `127.0.0.1:<random>`
- **Per-rule TCP forwarding** via `tokio::io::copy_bidirectional` (full-duplex)
- **Structured logging**: `daemon.log` + `rules/<name>.log` with timestamps
- **Cross-platform paths**: XDG on Linux, AppData on Windows (via `directories` crate)
- **Graceful shutdown**: `CancellationToken` tree, cleanup on SIGINT/SIGTERM/stop command
- **Config auto-reload**: enable/disable/modify/remove notify daemon automatically
- **Log follow mode** (`-f`): 500ms polling, works on Linux and Windows
- Comprehensive test suite: 105 tests passed (53 Windows, 52 Linux/WSL2)

### Architecture

```
src/
├── main.rs      # Entry point
├── cli.rs       # CLI parsing & command handlers
├── config.rs    # TOML config read/write
├── forward.rs   # TCP forwarding engine
├── daemon.rs    # Daemon lifecycle & rule management
├── control.rs   # TCP JSON control server + client
├── state.rs     # Runtime state file
├── logs.rs      # Log utilities
└── error.rs     # Error types (thiserror)
```

### Removed

- Tauri 2 GUI desktop application
- React + TypeScript WebUI frontend
- Axum HTTP API server
- All prior `core/`, `frontend/`, `gui/`, `server/` code
- `port.json` config (replaced by TOML)
- `run.bat` launcher

---

## [0.3.0] — 2026-05-16

### Changed

- **Architecture refactored**: single-file Rust core (~800 lines) + embedded WebUI + Tauri 2 GUI
- **CLI-first approach**: added `add`, `modify`, `list`, `delete`, `start`, `stop` commands
- Configuration moved from project root to user home directory
- `list` command gained `--verbose/-v` flag for full field display and record count
- `add`/`modify` gained `--source-address` support
- WebUI changed from mandatory to on-demand (`serve` command with `--port`/`--host`)

### Added

- Release CI/CD pipeline: Windows GUI portable exe + Linux CLI binary
- Linux musl static compilation guide (GLIBC compatibility)

### Removed

- Legacy `core/`, `frontend/`, `packaging/` codebases
- Old CI workflows

---

## [0.2.0] — 2026-05-04

### Added

- **Tauri 2 GUI desktop application** with embedded Axum server
- Startup synchronization for GUI main process
- GUI included in release workflow
- `CLAUDE.md` project documentation

### Changed

- Fixed clippy warnings across the codebase
- Finalized project directory structure
- Tauri build configuration adjusted for binary artifacts

---

## [0.1.0] — 2026-05-04

### Added

- Initial release: **PortHannis port forwarding manager**
- Rust backend with Axum HTTP API server:
  - `core/src/config.rs` — JSON configuration management
  - `core/src/forwarder.rs` — TCP port forwarding engine
  - `core/src/manager.rs` — rule lifecycle management
  - `core/src/logger.rs` — structured logging system
  - `core/src/api/` — REST API endpoints (control, entries, logs)
  - `core/src/models.rs` — shared data models
  - `core/src/error.rs` — error handling
- React + TypeScript frontend (`frontend/`):
  - Rule management UI (list, add, edit, delete)
  - Confirm dialog, empty state, layout components
  - API client with typed interfaces
- CI/CD workflows (GitHub Actions): `ci.yml`, `release.yml`

---

[0.4.0]: https://github.com/HannisLee/PortHannis/compare/v0.3.0...v0.4.0
[0.3.0]: https://github.com/HannisLee/PortHannis/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/HannisLee/PortHannis/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/HannisLee/PortHannis/releases/tag/v0.1.0
