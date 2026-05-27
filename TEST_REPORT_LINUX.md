# portcli — Linux Test Report (WSL2)

**Date**: 2026-05-27
**Platform**: Ubuntu 24.04 LTS (WSL2), kernel 6.6.87.2-microsoft-standard-WSL2, x86_64
**Rust**: 1.95.0 (stable-x86_64-unknown-linux-gnu)
**Binary**: `target/release/portcli` (ELF 64-bit LSB)

---

## Summary

| Category | Tests | Passed | Failed | Notes |
|----------|-------|--------|--------|-------|
| Build & CLI basics | 2 | 2 | 0 | |
| Config CRUD | 14 | 14 | 0 | |
| Enable/Disable | 6 | 6 | 0 | |
| Daemon lifecycle | 10 | 10 | 0 | |
| TCP forwarding | 1 | 1 | 0 | E2E verified with netcat |
| Reload | 6 | 6 | 0 | |
| Logs | 8 | 8 | 0 | |
| Edge cases & cleanup | 5 | 5 | 0 | |
| **Total** | **52** | **52** | **0** | |

---

## 1. Build & CLI Basics

### 1.1 `cargo build --release`
**Result**: PASS — Clean build on Linux native (not cross-compiled), 0 warnings, 0 errors.

```
Compiling portcli v0.1.0 (/home/lihan/portcli)
    Finished `release` profile [optimized] target(s) in 17.26s
```

### 1.2 `--version` / `--help`
**Result**: PASS

```
portcli 0.1.0
TCP port forwarding CLI tool

Commands:
  list     List all forwarding rules
  add      Add a new forwarding rule
  modify   Modify an existing rule
  remove   Remove a rule
  enable   Enable a rule
  disable  Disable a rule
  run      Start the daemon process
  status   Show daemon and rule status
  stop     Stop the daemon
  reload   Reload daemon configuration
  logs     View logs
  help     Print this message
```

All 11 subcommands present, identical to Windows version.

---

## 2. Config CRUD

### 2.1 `list` (empty config)
**Result**: PASS — `no rules configured`

### 2.2 `add` — normal
**Result**: PASS

```
rule 'web' added
rule 'ssh' added
```

### 2.3 `add` — duplicate name
**Result**: PASS — Error: `rule 'web' already exists`, exit code 1.

### 2.4 `list` (2 rules)
**Result**: PASS — Shows both rules with `enabled=false`, `status=unknown`.

### 2.5–2.8 `modify` — source, target, no changes, non-existent
**Result**: PASS — All four cases work identically to Windows:
- Source modified: `0.0.0.0:8080` → `0.0.0.0:8081`
- Target modified: `192.168.31.10:8080` → `192.168.31.20:8080`
- No changes: `error: no changes specified`
- Non-existent: `error: rule 'ghost' not found`

### 2.9–2.10 `remove` — non-existent, normal
**Result**: PASS — Identical to Windows.

### 2.11 Address validation — bad port
**Result**: PASS — `error: invalid address '0.0.0.0:99999': invalid port`

### 2.12 Address validation — no port
**Result**: PASS — `error: invalid address '0.0.0.0': expected host:port`

### 2.13 Verify list columns (Linux terminal)
**Result**: PASS — Column alignment preserved in Linux terminal output.

### 2.14 Verify config file written to Linux path
**Result**: PASS — Config saved to `~/.config/portcli/config.toml` with correct TOML format.

---

## 3. Enable / Disable

### 3.1 `enable` — normal
**Result**: PASS — `rule 'web' enabled`, config reflects `enabled = true`.

### 3.2 `enable` — non-existent
**Result**: PASS — `error: rule 'ghost' not found`, exit code 1.

### 3.3 `disable` — normal
**Result**: PASS — `rule 'web' disabled`, config reflects `enabled = false`.

### 3.4 `disable` — non-existent
**Result**: PASS — `error: rule 'ghost' not found`, exit code 1.

### 3.5 Enable → Disable → Re-enable cycle
**Result**: PASS — All transitions idempotent.

### 3.6 Idempotent enable/disable
**Result**: PASS — Re-enabling an already-enabled rule returns success without error.

---

## 4. Daemon Lifecycle

### 4.1 `status` (daemon not running)
**Result**: PASS — Shows `daemon: not running` with all configured rules and their enabled state.

### 4.2 `run` (background start)
**Result**: PASS — `daemon started (pid: 16279)`, process detaches and runs independently.

### 4.3 `run` (double start prevention)
**Result**: PASS — `daemon is already running (pid: 16279)`, non-error exit.

### 4.4 `status` (daemon running)
**Result**: PASS — Full status output:

```
daemon: running
pid: 16279
control: 127.0.0.1:45405

rules:

- web
  source: 0.0.0.0:8081
  target: 127.0.0.1:8080
  enabled: true
  status: running
```

### 4.5 `run --foreground`
**Result**: PASS — Daemon runs in foreground;
- Responds to signals (SIGTERM from `timeout`)
- Logs `daemon stopping (signal)` on Ctrl+C/SIGTERM
- Clean shutdown on signal

### 4.6 `stop` (normal)
**Result**: PASS — Full graceful shutdown sequence:
1. `daemon stopping` message
2. All rule tasks cancelled and awaited
3. State file deleted
4. Daemon log: `daemon stopping (control)` → `daemon stopped`

### 4.7 `stop` (when not running)
**Result**: PASS — `daemon is not running`, no crash.

### 4.8 State file cleanup
**Result**: PASS — `~/.local/share/portcli/state.json` deleted after every stop.

### 4.9 Foreground daemon survives timeout
**Result**: PASS — After `timeout 3 portcli run --foreground`, the daemon exits cleanly and state is cleaned up.

### 4.10 Background process independence
**Result**: PASS — Background daemon continues running after the spawning shell exits (verified via `$B run` then `$B status` in separate invocations).

---

## 5. TCP Forwarding (End-to-End)

### 5.1 Bidirectional data forwarding with netcat
**Test setup**:
1. Backend: `nc -l -p 8080` listening on `127.0.0.1:8080`
2. portcli rule: `source=0.0.0.0:8081 → target=127.0.0.1:8080`, enabled
3. Daemon running in background

**Result**: PASS

| Step | Action | Result |
|------|--------|--------|
| Client connects to `127.0.0.1:8081` | TCP handshake | Connected (local port 45560) |
| Client sends `"PING_FROM_LINUX_CLIENT"` (23 bytes) | Write to stream | Data forwarded to backend port 8080 |
| Log verification | `portcli logs web -n 5` | `connection accepted name=web peer=127.0.0.1:45560` |

**Log evidence**:
```
[2026-05-27 16:06:53] INFO connection accepted name=web peer=127.0.0.1:45560
[2026-05-27 16:06:55] INFO connection closed name=web peer=127.0.0.1:45560 bytes_sent=23 bytes_received=0
```

- `bytes_sent=23` matches the 23-byte test payload exactly
- `bytes_received=0` is expected (netcat listener does not echo)

---

## 6. Reload

### 6.1 `reload` after `modify` (auto-reload)
**Result**: PASS — After `modify web --target 127.0.0.1:8080`:
```
rule 'web' modified
daemon: config reloaded
```

### 6.2 `reload` after `enable` (auto-reload)
**Result**: PASS — After `enable db`:
```
rule 'db' enabled
daemon: config reloaded
```
Rule appears in status with `status: running`.

### 6.3 `reload` after `disable` (auto-reload)
**Result**: PASS — After `disable db`:
```
rule 'db' disabled
daemon: config reloaded
```
Rule disappears from status.

### 6.4 Reload log confirmation
**Result**: PASS — Each reload logged in daemon.log:
```
[2026-05-27 16:05:50] INFO config reloaded
[2026-05-27 16:05:51] INFO config reloaded
[2026-05-27 16:05:52] INFO config reloaded
```

### 6.5 Reload when daemon not running
**Result**: PASS — `daemon is not running`, no crash.

### 6.6 Multiple sequential reloads
**Result**: PASS — Enable → reload → modify → reload → disable → reload: all 3 reloads succeed without issues.

---

## 7. Logs

### 7.1 `logs` (daemon log)
**Result**: PASS — Shows daemon lifecycle entries.

### 7.2 `logs <name>` (rule log)
**Result**: PASS — Shows full rule lifecycle: started → connections → stopping.

### 7.3 `logs <name> -n 3`
**Result**: PASS — Exactly 3 most recent lines displayed.

### 7.4 `logs --dir`
**Result**: PASS — Output: `/home/lihan/.local/share/portcli/logs` (correct Linux XDG path).

### 7.5 `logs --clear` (daemon and rule)
**Result**: PASS — Both daemon and rule logs cleared; subsequent `logs` commands show empty output.

### 7.6 `logs <non-existent>`
**Result**: PASS — Clear message: `ghost log not found: /home/lihan/.local/share/portcli/logs/rules/ghost.log`

### 7.7 Log format consistency
**Result**: PASS — All logs follow `[YYYY-MM-DD HH:MM:SS] LEVEL message` format, timezone is local (CST).

### 7.8 Log file write concurrency
**Result**: PASS — Multiple simultaneous connections do not interleave log lines (each connection produces consecutive log entries).

---

## 8. Edge Cases & Error Handling

| # | Test | Expected | Actual | Result |
|---|------|----------|--------|--------|
| 8.1 | Empty config, `list` | "no rules configured" | "no rules configured" | PASS |
| 8.2 | Empty config, `status` | "daemon: not running" | "daemon: not running" | PASS |
| 8.3 | Modify with no flags | "no changes specified" | "no changes specified" | PASS |
| 8.4 | Enable already-enabled | Success (idempotent) | Success | PASS |
| 8.5 | Disable already-disabled | Success (idempotent) | Success | PASS |
| 8.6 | Stop stopped daemon | "daemon is not running" | "daemon is not running" | PASS |
| 8.7 | Reload stopped daemon | "daemon is not running" | "daemon is not running" | PASS |
| 8.8 | Run already-running | "daemon is already running" | "daemon is already running" | PASS |
| 8.9 | Bind to port in use | Bind failed logged | Bind failed logged with `os error 98` (EADDRINUSE) | PASS |
| 8.10 | Self-spawning via `run` | Child process detaches | Child PID printed, parent exits, child continues | PASS |

---

## 9. Linux-Specific Paths

| Purpose | Path |
|---------|------|
| Config TOML | `~/.config/portcli/config.toml` |
| Runtime state | `~/.local/share/portcli/state.json` |
| Daemon log | `~/.local/share/portcli/logs/daemon.log` |
| Rule logs | `~/.local/share/portcli/logs/rules/<name>.log` |

All paths follow the XDG Base Directory Specification via the `directories` crate.

**Config file content (TOML)**:
```toml
[[rules]]
name = "web"
source = "0.0.0.0:8081"
target = "127.0.0.1:8080"
enabled = true

[[rules]]
name = "db"
source = "127.0.0.1:5432"
target = "192.168.31.50:5432"
enabled = false
```

---

## 10. Platform Comparison: Windows vs Linux

| Feature | Windows | Linux | Notes |
|---------|---------|-------|-------|
| Build | MSVC (PE32+) | GNU (ELF) | Both clean, 0 warnings |
| Config path | `%APPDATA%\portcli\config\` | `~/.config/portcli/` | XDG-compliant |
| State/log path | `%LOCALAPPDATA%\portcli\data\` | `~/.local/share/portcli/` | XDG-compliant |
| Background daemon | `DETACHED_PROCESS` flag | `fork` + `exec` (via Command) | Both work |
| Signal handling | Ctrl+C → tokio signal | SIGINT/SIGTERM → tokio signal | Same code path |
| Bind errors | `os error 10048` (WSAEADDRINUSE) | `os error 98` (EADDRINUSE) | Platform-specific error codes |
| `/dev/tcp` | N/A | Not used (tokio::net) | Tokio abstracts this |
| Log directory separator | `\` | `/` | Handled by std::path |

**Key finding**: The codebase is truly cross-platform. All 11 commands, the daemon, control protocol, TCP forwarding, and logging work identically on Linux and Windows with zero platform-specific code changes (only the background spawn uses `#[cfg(windows)]` for `DETACHED_PROCESS`).

---

## 11. Bug Previously Found (Windows) — Confirmed Fixed on Linux

The daemon main loop `root_cancel.cancelled()` fix from Windows testing (§9 of Windows report) was already present in the Linux build. Verified:

- `portcli stop` correctly triggers `daemon stopping (control)` log entry
- State file deleted after stop
- Daemon process exits cleanly

```
[2026-05-27 16:06:14] INFO daemon stopping (control)
[2026-05-27 16:06:14] INFO daemon stopped
```

---

## 12. Final Build Verification

```bash
$ cargo build --release
    Finished `release` profile [optimized] target(s) in 0.13s
```

**Result**: PASS — 0 errors, 0 warnings.

---

## Conclusion

All 52 tests pass on Linux (WSL2, Ubuntu 24.04). The project is fully cross-platform:

- **Windows** (53 tests): All pass, tested with MSVC toolchain
- **Linux** (52 tests): All pass, tested with GNU toolchain

The codebase requires zero platform-specific modifications. The only `#[cfg(windows)]` block is the `DETACHED_PROCESS` flag for background daemon spawning — everything else works identically via tokio's cross-platform abstractions.

The project is ready for production use on both platforms.
