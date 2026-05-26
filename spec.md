# PortCLI Specification

## 1. Scope

PortHannis will be refactored into a pure command-line TCP port forwarding tool.

- Repository name: `PortHannis`
- Binary name: `portcli`
- Supported platforms: Linux, Windows, macOS
- User interface: CLI only
- Runtime model: background daemon controlled by CLI commands

The project must remove the desktop GUI, Wails frontend, WebUI, and system tray behavior from the final product.

## 2. Core Concepts

### Rule Name

Rules are identified only by name. There is no rule ID in code, config, CLI output, logs API, or user-facing behavior.

Rules names must be unique within `port.json`.

If a user adds a rule without `--name`, the CLI generates the first available sequential name:

- `name1`
- `name2`
- `name3`

Names are reused only if the lower numbered name is currently absent.

### Rule

A rule describes one TCP forwarding endpoint:

- `sourceHost`: local listen host, for example `0.0.0.0` or `127.0.0.1`
- `localPort`: local listen port
- `targetHost`: target host or IP
- `targetPort`: target port
- `enabled`: whether the daemon should run the rule
- `logPath`: per-rule log file path

## 3. Configuration

The only configuration file is `port.json`.

The top-level JSON value is an object. Each key is a rule name and each value is a rule object.

Example:

```json
{
  "name1": {
    "sourceHost": "0.0.0.0",
    "localPort": 8080,
    "targetHost": "192.168.1.100",
    "targetPort": 3000,
    "enabled": true,
    "logPath": ""
  }
}
```

Rules:

- `port.json` must not contain comments.
- `logPath` is optional for hand-written config files.
- If `logPath` is empty or missing, `portcli add` and `portcli enable <name>` must generate a default log path and write it back to `port.json`.
- If `logPath` is already set, the CLI must preserve it.
- The default log path should be under the config directory, inside `logs/`, with a file name derived from the rule name.

## 4. Config Directory

Use the platform-appropriate user config directory.

Recommended defaults:

- Linux: `$XDG_CONFIG_HOME/porthannis` or `~/.config/porthannis`
- Windows: `%APPDATA%\porthannis`
- macOS: `~/Library/Application Support/porthannis`

The config file path is:

```text
<config-dir>/port.json
```

The default logs directory is:

```text
<config-dir>/logs/
```

## 5. CLI Commands

### add

Adds a rule to `port.json`.

```bash
portcli add --listen 0.0.0.0:8080 --target 192.168.1.100:3000
portcli add --name web --listen 127.0.0.1:9000 --target 10.0.0.5:22
```

Behavior:

- Fails if the provided name already exists.
- Generates `nameN` when `--name` is omitted.
- Parses `--listen` into `sourceHost` and `localPort`.
- Parses `--target` into `targetHost` and `targetPort`.
- Sets `enabled` to `true` by default.
- Generates and stores `logPath`.

### list

Lists configured rules.

```bash
portcli list
```

Output must include:

- rule name
- listen address
- target address
- enabled state
- log path

Output must not include an ID.

### enable

Enables a rule by name.

```bash
portcli enable name1
```

Behavior:

- Sets `enabled` to `true`.
- Generates and stores `logPath` if missing.
- Does not require the daemon to be running.

### disable

Disables a rule by name.

```bash
portcli disable name1
```

Behavior:

- Sets `enabled` to `false`.
- Does not require the daemon to be running.

### remove

Removes a rule by name.

```bash
portcli remove name1
```

Behavior:

- Deletes the rule from `port.json`.
- Does not delete the log file unless a future command explicitly supports that behavior.

### run

Starts the background daemon.

```bash
portcli run
```

Behavior:

- Starts a background daemon process and returns.
- If a daemon is already running, prints a clear message and exits successfully.
- The daemon loads `port.json` and starts all rules with `enabled=true`.
- The daemon keeps running until stopped by `portcli stop` or by process termination.

### status

Queries daemon status.

```bash
portcli status
```

Behavior:

- Reports whether the daemon is running.
- If running, reports daemon PID and per-rule runtime status.
- Rule status output uses rule names only.

### stop

Stops the background daemon.

```bash
portcli stop
```

Behavior:

- Sends a stop request to the daemon.
- The daemon stops all running forwarding rules.
- The daemon removes its state file before exiting.
- If no daemon is running, prints a clear message and exits successfully.

### logs

Reads a rule log by name.

```bash
portcli logs name1 --limit 100
portcli logs name1 --follow
```

Behavior:

- Reads the `logPath` configured for the rule.
- `--limit` returns the newest N entries.
- `--follow` streams appended log entries.

### clear-logs

Clears a rule log by name.

```bash
portcli clear-logs name1
```

Behavior:

- Truncates the file at the rule's `logPath`.
- Fails clearly if the rule does not exist.

## 6. Daemon Control

The daemon control mechanism should be cross-platform.

Recommended design:

- The daemon listens on `127.0.0.1` using a randomly selected control port.
- A state file stores:
  - PID
  - control port
  - random token
- `status` and `stop` read the state file and call the local control endpoint.
- Control requests must include the token.
- If the state file exists but the daemon does not respond, CLI commands should report a stale state and remove or ignore the stale state safely.

The daemon should expose at least:

- status endpoint or command
- stop endpoint or command

The control interface is local-only and not a public WebUI.

## 7. Port Validation

- Local listen port: `1-65535`
- Target port: `1-65535`
- The CLI must not block privileged ports such as `80` or `443`.
- Permission failures are delegated to the OS and should be reported with the original listen error.

## 8. Forwarding

Only TCP forwarding is required.

For each enabled rule:

- Listen on `sourceHost:localPort`.
- Dial `targetHost:targetPort` for each incoming connection.
- Copy bytes in both directions until either side closes.
- Record a log entry per connection.

## 9. Logging

Each rule has an independent log file.

Log behavior:

- Default maximum file size: 10 MB.
- When the file reaches the limit, keep newest logs by rotating or truncating according to implementation.
- Log entries should include:
  - timestamp
  - source client address
  - bytes in
  - bytes out
  - status

## 10. Build

The final build output should be named:

- Linux: `portcli`
- Windows: `portcli.exe`
- macOS: `portcli`

Cross-platform build targets should include:

- `linux/amd64`
- `linux/arm64`
- `windows/amd64`
- `windows/arm64`
- `darwin/amd64`
- `darwin/arm64`
