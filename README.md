# PortHannis

PortHannis is being refactored into `portcli`, a lightweight cross-platform TCP port forwarding CLI.

The repository name remains `PortHannis`; the release binary is `portcli`.

## Status

This repository is currently in a CLI refactor phase. The target product removes the previous desktop GUI, Wails frontend, WebUI, and system tray behavior, and keeps a command-line workflow with a background daemon.

See [spec.md](spec.md) for the full product specification and [version.md](version.md) for release history.

## Features

- Single-file executable
- Cross-platform support: Linux, Windows, macOS
- Background daemon started by `portcli run`
- Runtime status query through `portcli status`
- Graceful daemon shutdown through `portcli stop`
- TCP forwarding from local listen address to target address
- JSON configuration in one `port.json` file
- Rule names instead of IDs
- Default rule names from `name1`, `name2`, `name3`
- Per-rule logs with configurable `logPath`

## Quick Start

Add a forwarding rule:

```bash
portcli add --listen 0.0.0.0:8080 --target 192.168.1.100:3000
```

Add a named rule:

```bash
portcli add --name web --listen 127.0.0.1:9000 --target 10.0.0.5:22
```

Start the background daemon:

```bash
portcli run
```

Check status:

```bash
portcli status
```

Stop the daemon:

```bash
portcli stop
```

## Commands

```bash
portcli add --listen 0.0.0.0:8080 --target 192.168.1.100:3000
portcli add --name web --listen 127.0.0.1:9000 --target 10.0.0.5:22

portcli list
portcli enable name1
portcli disable name1
portcli remove name1

portcli run
portcli status
portcli stop

portcli logs name1 --limit 100
portcli logs name1 --follow
portcli clear-logs name1
```

## Configuration

The only configuration file is `port.json`.

The top-level JSON object maps rule names to rule objects. There are no rule IDs.

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

`logPath` may be omitted or left empty in hand-written config. `portcli add` and `portcli enable <name>` fill it automatically when missing.

Default config locations:

| Platform | Config path |
|----------|-------------|
| Linux | `$XDG_CONFIG_HOME/porthannis/port.json` or `~/.config/porthannis/port.json` |
| Windows | `%APPDATA%\porthannis\port.json` |
| macOS | `~/Library/Application Support/porthannis/port.json` |

Default logs are stored under:

```text
<config-dir>/logs/
```

## Port Policy

`portcli` accepts local and target ports from `1` to `65535`.

Privileged ports such as `80` and `443` are not blocked by the CLI. Permission checks are left to the operating system.

## Build Targets

Final release artifacts should use these names:

| Platform | Binary |
|----------|--------|
| Linux | `portcli` |
| Windows | `portcli.exe` |
| macOS | `portcli` |

Target platforms:

- `linux/amd64`
- `linux/arm64`
- `windows/amd64`
- `windows/arm64`
- `darwin/amd64`
- `darwin/arm64`

## Development

Requirements:

- Go 1.22+

The final CLI implementation should prefer the Go standard library unless an external dependency provides clear value.

Before creating a new release tag, update:

- [version.md](version.md)
- [README.md](README.md)

After creating a new release tag, verify both files reflect the released version.

## License

MIT
