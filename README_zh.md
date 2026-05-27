# portcli

一个用 Rust 编写的跨平台 TCP 端口转发 CLI 工具。通过 CLI 管理转发规则，由后台守护进程执行转发——无需 root 权限。

[![Rust](https://img.shields.io/badge/rust-1.70+-orange.svg)](https://www.rust-lang.org)
[![Platform](https://img.shields.io/badge/platform-linux%20%7C%20windows-blue.svg)](#)
[![Tests](https://img.shields.io/badge/tests-105%2F105%20passed-brightgreen.svg)](#)

## 功能特性

- 通过 CLI 管理 TCP 转发规则：添加、修改、删除、启用、禁用
- 后台守护进程持久化执行转发规则
- 本地 TCP JSON 控制协议，用于与守护进程通信
- 按规则和守护进程级别分别记录日志
- 跨平台：遵循 Linux (XDG) 和 Windows (AppData) 路径规范
- 无需特权端口或 root 权限

## 安装

需要 Rust 1.70 或更高版本。

```bash
cargo build --release
```

编译产物位于 `target/release/portcli`（Linux）或 `target\release\portcli.exe`（Windows）。建议将其加入 `PATH` 方便使用。

## 快速开始

```bash
# 添加一条规则（默认禁用）
portcli add web --source 0.0.0.0:8080 --target 192.168.31.10:8080

# 启用规则
portcli enable web

# 在后台启动守护进程
portcli run

# 查看运行状态
portcli status
portcli logs web
```

---

## 命令参考

### `portcli list`

列出所有规则，显示名称、源地址、目标地址、启用状态和运行时状态。如果守护进程正在运行，会显示实时状态；否则显示 `unknown`。

### `portcli add <name> --source <addr> --target <addr>`

添加一条新规则。名称必须唯一，地址必须是合法的 `host:port` 格式。默认 `enabled = false`。

```bash
portcli add web --source 0.0.0.0:8080 --target 192.168.31.10:8080
```

### `portcli modify <name> [--source <addr>] [--target <addr>]`

修改已有规则。只修改传入的字段。如果守护进程正在运行，会自动通知其重新加载配置。

```bash
portcli modify web --source 0.0.0.0:8081
portcli modify web --target 192.168.31.20:8080
```

### `portcli remove <name>`

删除规则。如果守护进程正在运行，会自动重新加载并停止该规则的转发。

### `portcli enable <name>`

将规则设为 `enabled = true`。如果守护进程正在运行，自动重新加载并启动该规则。

### `portcli disable <name>`

将规则设为 `enabled = false`。如果守护进程正在运行，自动重新加载并停止该规则。

### `portcli run`

在后台启动守护进程。守护进程读取配置文件并启动所有已启用的规则。

```bash
portcli run --foreground   # 前台运行（调试时使用）
```

### `portcli status`

显示守护进程是否运行、PID、控制地址以及每条规则的运行时状态。失败的规则会附带操作系统错误信息。

### `portcli stop`

优雅地停止守护进程。所有转发任务被取消，运行时状态文件被删除，进程退出。

### `portcli reload`

通知正在运行的守护进程重新从磁盘加载配置文件。所有规则会被停止，然后根据当前配置重新启动。

### `portcli logs [name] [-n <lines>] [-f] [--clear] [--dir]`

查看和管理日志文件。

| 用法 | 说明 |
|------|------|
| `portcli logs` | 显示 daemon 日志最后 100 行 |
| `portcli logs <name>` | 显示某条规则日志最后 100 行 |
| `-n <N>` / `--lines <N>` | 显示最后 N 行（默认 100） |
| `-f` / `--follow` | 持续跟踪日志输出（每 500ms 轮询，Ctrl+C 退出） |
| `--clear` | 清空日志文件 |
| `--dir` | 显示日志目录路径 |

---

## 文件路径

### 配置文件 (TOML)

| 平台 | 路径 |
|------|------|
| Linux | `~/.config/portcli/config.toml` |
| Windows | `%APPDATA%\portcli\config\config.toml` |

### 日志与运行时状态

| 平台 | 路径 |
|------|------|
| Linux | `~/.local/share/portcli/logs/` |
| Windows | `%LOCALAPPDATA%\portcli\data\logs\` |

```
logs/
├── daemon.log          # 守护进程日志
└── rules/
    ├── web.log         # web 规则日志
    └── ssh.log         # ssh 规则日志
```

运行时状态文件 (`state.json`) 写入与日志相同的父目录，守护进程停止时自动删除。

### 配置文件示例

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

## 架构概览

```
CLI (cli.rs)
 │
 ├── 读写配置 ──────────────────► config.toml
 │
 ├── 控制命令 ──────────────────► Daemon (daemon.rs)
 │    (TCP JSON, 127.0.0.1)        │
 │                                 ├── 控制服务 (control.rs)
 │                                 ├── 转发任务 (forward.rs)
 │                                 └── 日志写入 (logs.rs)
 │
 └── 查看日志 ──────────────────► 日志文件
```

1. **配置**：规则以 TOML 格式存储，CLI 和守护进程共享同一文件。
2. **守护进程**：绑定随机的本地回环控制端口，为每条已启用规则启动 TCP 监听。
3. **转发**：每个入站连接生成一个 tokio 任务，通过 `tokio::io::copy_bidirectional` 实现全双工转发。
4. **控制协议**：CLI 通过 JSON 协议与守护进程通信，使用存储在状态文件中的随机 token 进行认证。
5. **日志**：守护进程事件和每条连接的详细信息写入带时间戳的日志文件。

---

## 测试端口转发

**1. 在目标端口启动一个后端监听：**

```bash
# Linux / WSL
nc -l 8080
```

```powershell
# Windows PowerShell
$l = [System.Net.Sockets.TcpListener]::new([System.Net.IPAddress]::Parse("127.0.0.1"), 8080)
$l.Start(); $c = $l.AcceptTcpClient()
# … 在 $c.GetStream() 上读写
```

**2. 添加并启用规则：**

```bash
portcli add test --source 127.0.0.1:9999 --target 127.0.0.1:8080
portcli enable test
```

**3. 启动守护进程：**

```bash
portcli run --foreground
```

**4. 通过转发端口连接：**

```bash
# Linux
echo "hello" | nc 127.0.0.1 9999
```

```powershell
# Windows PowerShell
$c = New-Object System.Net.Sockets.TcpClient("127.0.0.1", 9999)
# … 在 $c.GetStream() 上读写
```

**5. 在日志中验证：**

```bash
portcli logs test
# [2026-05-27 12:00:10] INFO connection accepted name=test peer=127.0.0.1:53422
# [2026-05-27 12:00:11] INFO connection closed name=test peer=127.0.0.1:53422 bytes_sent=5 bytes_received=0
```

---

## 项目结构

```
src/
├── main.rs      # 入口点
├── cli.rs       # CLI 参数解析 (clap derive) 和命令处理
├── config.rs    # TOML 配置文件读写
├── forward.rs   # TCP 转发逻辑 (tokio::io::copy_bidirectional)
├── daemon.rs    # 守护进程生命周期、规则管理、优雅退出
├── control.rs   # TCP JSON 控制协议（服务端 + 客户端）
├── state.rs     # 运行时状态文件 (JSON)
├── logs.rs      # 日志工具：路径、追加、读取、跟踪、清空
└── error.rs     # 错误类型定义 (thiserror)
```

---

## 当前限制 (MVP)

- 仅支持 TCP（不支持 UDP）
- 不支持 TLS 加密
- 未集成 systemd / Windows Service
- 无图形界面
- 不支持配置文件热重载（修改后需手动执行 `portcli reload`）
- 仅通过本地 token 进行认证，无额外安全机制
- 日志跟踪使用轮询方式（500ms 间隔），未使用原生文件监控 API

---

## 测试结果

| 平台 | 测试数 | 通过 |
|------|--------|------|
| Windows 11 (MSVC) | 53 | 53 |
| Ubuntu 24.04 / WSL2 (GNU) | 52 | 52 |

详细测试报告：[TEST_REPORT.md](TEST_REPORT.md) · [TEST_REPORT_LINUX.md](TEST_REPORT_LINUX.md)

---

## 许可证

MIT
