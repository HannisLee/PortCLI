# Version History

This file summarizes the visible Git tags and notable project history. Update it whenever a new release tag is created.

## Release Tags

| Tag | Date | Summary |
|-----|------|---------|
| `v0.1.0` | 2026-05-02 | First Go/Wails milestone with core config, TCP forwarding engine, tray/frontend integration, embedded WebUI, and build resources. |
| `v0.2.0` | 2026-05-04 | Rust/Tauri architecture milestone from the remote history, including Rust core, embedded WebUI, Tauri GUI, and finalized project structure. |
| `v0.2.1` | 2026-05-04 | CI fix release using `cargo-binstall` for faster Tauri CLI installation. |
| `v0.3.0` | 2026-05-07 | Release workflow milestone for Windows portable GUI executable and Linux headless binary; repository links corrected to `HannisLee/PortHannis`. |

## Notable History

### 2026-04-29 to 2026-05-02: Go/Wails Foundation

- Initialized the PortHannis repository and development workflow docs.
- Created the Wails + Svelte project structure.
- Added core config types and config manager.
- Implemented the TCP port forwarding engine.
- Added system tray integration and Svelte/Tailwind frontend.
- Added embedded WebUI server for browser-based management.
- Tagged `v0.1.0`.
- Added fixes for source host support, WebUI status, and runtime Wails detection.

### 2026-05-04 to 2026-05-07: Rust/Tauri Release Track

- Remote history includes a refactor to a Rust single-file core, embedded WebUI, and Tauri 2 GUI desktop app.
- Fixed clippy warnings and Tauri build configuration.
- Added GUI release workflow changes.
- Tagged `v0.2.0`, `v0.2.1`, and `v0.3.0`.
- Added Release CI/CD workflow and build optimizations.

### 2026-05-13 to 2026-05-25: CLI and Release Process Work

- Remote history includes a CLI-first refactor with WebUI started on demand.
- README was expanded with CLI command documentation.
- CLI commands were enhanced with `add`, `modify`, `list --verbose`, `delete`, `start`, and `stop` behavior.
- Config storage was moved to the user config directory.
- Release flow was updated for Windows GUI portable executable and Linux CLI artifacts.
- Linux musl static build documentation was added to address GLIBC compatibility.

### Current Refactor Target: PortCLI

The current specification targets a pure Go CLI product named `portcli`:

- Repository stays named `PortHannis`.
- Final product removes Wails, WebUI, frontend, and system tray behavior.
- `portcli run` starts a background daemon.
- `portcli status` queries daemon and rule status.
- `portcli stop` gracefully shuts down the daemon.
- `port.json` is the only config file.
- Rule names replace IDs completely.
- Default names start at `name1`.
- `logPath` is optional in hand-written JSON and is filled by the CLI when needed.

See `spec.md` for the authoritative implementation specification.

## Maintenance Rule

Whenever a new tag is created:

1. Add the tag, date, and summary to the Release Tags table.
2. Add notable changes to the history sections when useful.
3. Update `README.md` so usage, configuration, and release notes match the tagged behavior.
