# portcli

`portcli` 是一个用 Rust 编写的跨平台 TCP 端口转发命令行工具。它用 CLI 管理转发规则，用后台守护进程执行转发；普通高位端口不需要 root 或管理员权限。

[![Rust](https://img.shields.io/badge/rust-1.70+-orange.svg)](https://www.rust-lang.org)
[![CI](https://github.com/HannisLee/PortHannis/actions/workflows/ci.yml/badge.svg)](https://github.com/HannisLee/PortHannis/actions/workflows/ci.yml)
[![Platform](https://img.shields.io/badge/platform-linux%20%7C%20windows-blue.svg)](#)
[![Tests](https://img.shields.io/badge/tests-105%2F105%20passed-brightgreen.svg)](#)

## 功能特性

- 通过 CLI 管理 TCP 转发规则：添加、修改、删除、启用、禁用
- 后台守护进程持久运行，按配置启动所有已启用规则
- 本机 TCP JSON 控制协议，用于 `status`、`stop`、`reload`
- 每条规则独立日志，同时保留守护进程日志
- 遵循 Linux XDG 和 Windows AppData 目录规范
- 支持 Linux 和 Windows

## 安装

### 从 GitHub Releases 下载

从 [GitHub Releases](https://github.com/HannisLee/PortHannis/releases) 下载对应平台的预编译二进制文件。

**Linux 静态链接版本：**

```bash
wget https://github.com/HannisLee/PortHannis/releases/download/v0.4.0/portcli-v0.4.0-x86_64-unknown-linux-musl.tar.gz
tar -xzf portcli-v0.4.0-x86_64-unknown-linux-musl.tar.gz
chmod +x portcli
sudo mv portcli /usr/local/bin/portcli
portcli --help
```

musl 版本是静态链接构建，通常可在 Ubuntu、Debian、CentOS、Rocky、Alpine 等发行版上运行。

```bash
ldd ./portcli
# 静态链接版本通常显示 not a dynamic executable
```

**Windows：**

1. 从 Releases 页面下载 `portcli-v0.4.0-x86_64-pc-windows-msvc.zip`
2. 解压 `portcli.exe` 到固定目录，例如 `C:\Tools\portcli\`
3. 将该目录加入 `PATH`
4. 打开 PowerShell 验证：

```powershell
portcli --help
```

### 从源码编译

需要 Rust 1.70 或更高版本。

```bash
git clone https://github.com/HannisLee/PortHannis.git
cd PortHannis
cargo build --release
```

编译产物位于：

- Linux：`target/release/portcli`
- Windows：`target\release\portcli.exe`

## 快速开始

下面的例子把本机 `127.0.0.1:9999` 转发到本机已有服务 `127.0.0.1:8080`。

```bash
# 1. 添加规则。新规则默认是禁用状态
portcli add local-web --source 127.0.0.1:9999 --target 127.0.0.1:8080

# 2. 启用规则
portcli enable local-web

# 3. 后台启动守护进程
portcli run

# 4. 查看状态
portcli status

# 5. 通过源端口访问目标服务
curl http://127.0.0.1:9999

# 6. 查看这条规则的日志
portcli logs local-web -n 20
```

停止转发：

```bash
portcli disable local-web
portcli stop
```

## 常用场景示例

### 1. 把本机开发服务暴露给局域网

适合手机、同事电脑或测试设备访问你电脑上的开发服务。

```bash
# 假设你的开发服务监听在 127.0.0.1:3000
portcli add dev-web --source 0.0.0.0:8080 --target 127.0.0.1:3000
portcli enable dev-web
portcli run
```

局域网内其他设备访问：

```text
http://你的电脑局域网IP:8080
```

如果改了端口或目标服务地址：

```bash
portcli modify dev-web --source 0.0.0.0:8081
portcli modify dev-web --target 127.0.0.1:5173
```

`modify` 会在守护进程运行时自动触发配置重载。

### 2. 转发 SSH 端口

适合把远端或局域网机器的 SSH 服务映射到本机端口。

```bash
portcli add ssh-box --source 127.0.0.1:2222 --target 192.168.31.20:22
portcli enable ssh-box
portcli run
ssh -p 2222 user@127.0.0.1
```

只允许本机访问时，`--source` 使用 `127.0.0.1:<端口>`；需要局域网访问时，使用 `0.0.0.0:<端口>`。

### 3. 同时管理多条规则

```bash
portcli add web --source 0.0.0.0:8080 --target 192.168.31.10:80
portcli add api --source 0.0.0.0:9000 --target 192.168.31.11:9000
portcli add ssh --source 127.0.0.1:2222 --target 192.168.31.20:22

portcli enable web
portcli enable api
portcli enable ssh
portcli run

portcli list
portcli status
```

临时停掉其中一条：

```bash
portcli disable api
```

彻底删除规则：

```bash
portcli remove api
```

### 4. 前台运行用于调试

前台模式不会把守护进程放到后台，适合在终端中观察启动失败、端口占用等问题。

```bash
portcli run --foreground
```

另开一个终端查看状态：

```bash
portcli status
portcli logs -n 50
```

### 5. 查看和跟踪日志

```bash
# 守护进程日志最后 100 行
portcli logs

# 某条规则最后 50 行
portcli logs web -n 50

# 持续跟踪某条规则日志
portcli logs web -f

# 查看日志目录
portcli logs --dir

# 清空规则日志或守护进程日志
portcli logs web --clear
portcli logs --clear
```

规则日志里会记录连接建立、目标连接失败、转发结束和字节数等信息。

### 6. 手动编辑配置后重新加载

CLI 命令 `enable`、`disable`、`modify`、`remove` 在守护进程运行时会自动通知守护进程重新加载配置。只有你手动改了 `config.toml` 时，才需要执行：

```bash
portcli reload
```

如果守护进程没有运行，`reload` 会提示 `daemon is not running`。

### 7. 完整连通性测试

Linux / WSL 下可以用 `nc` 做一个最小验证。

```bash
# 终端 A：启动目标服务
nc -l 8080
```

```bash
# 终端 B：创建 9999 -> 8080 的转发
portcli add test --source 127.0.0.1:9999 --target 127.0.0.1:8080
portcli enable test
portcli run
echo "hello" | nc 127.0.0.1 9999
```

然后查看日志：

```bash
portcli logs test -n 10
# [2026-05-27 12:00:10] INFO connection accepted name=test peer=127.0.0.1:53422
# [2026-05-27 12:00:11] INFO connection closed name=test peer=127.0.0.1:53422 bytes_sent=6 bytes_received=0
```

## 命令参考

### 全局选项

| 用法 | 说明 |
| --- | --- |
| `portcli --version` / `portcli -V` | 显示当前版本号 |
| `portcli --help` / `portcli -h` | 显示顶层帮助、常用示例和所有子命令 |
| `portcli <command> --help` | 查看某个子命令的详细说明和示例 |

```bash
portcli --version
portcli --help
portcli add --help
portcli logs --help
```

### `portcli list`

列出所有规则，包含规则名、源地址、目标地址、启用状态和运行时状态。如果守护进程没有运行，运行时状态通常显示为 `unknown`。

```bash
portcli list
```

### `portcli add <name> --source <addr> --target <addr>`

添加新规则。规则名必须唯一；地址必须是 `host:port` 格式；新规则默认 `enabled = false`。

```bash
portcli add web --source 0.0.0.0:8080 --target 192.168.31.10:80
```

### `portcli modify <name> [--source <addr>] [--target <addr>]`

修改已有规则。只会更新传入的字段。守护进程运行时会自动重新加载配置。

```bash
portcli modify web --source 0.0.0.0:8081
portcli modify web --target 192.168.31.20:80
```

### `portcli remove <name>`

删除规则。守护进程运行时会自动重新加载配置，并停止该规则的转发任务。

```bash
portcli remove web
```

### `portcli enable <name>`

启用规则。守护进程运行时会自动重新加载配置，并启动对应转发。

```bash
portcli enable web
```

### `portcli disable <name>`

禁用规则。守护进程运行时会自动重新加载配置，并停止对应转发。

```bash
portcli disable web
```

### `portcli run [--foreground]`

启动守护进程。默认在后台运行；`--foreground` 用于前台调试。

```bash
portcli run
portcli run --foreground
```

如果守护进程已经在运行，再次执行 `portcli run` 会显示已有进程 PID，不会重复启动。

### `portcli status`

显示守护进程是否运行、PID、控制地址，以及运行中的规则状态。规则启动失败时会显示错误信息，例如端口被占用或目标地址不可达。

```bash
portcli status
```

### `portcli stop`

优雅停止守护进程，取消所有转发任务，并删除运行时状态文件。

```bash
portcli stop
```

### `portcli reload`

通知正在运行的守护进程重新读取配置文件。守护进程会停止当前规则任务，然后按最新配置重新启动已启用规则。

```bash
portcli reload
```

### `portcli logs [name] [-n <lines>] [-f] [--clear] [--dir]`

查看和管理日志。

| 用法 | 说明 |
| --- | --- |
| `portcli logs` | 查看守护进程日志最后 100 行 |
| `portcli logs <name>` | 查看指定规则日志最后 100 行 |
| `portcli logs -n 20` | 查看守护进程日志最后 20 行 |
| `portcli logs web -n 20` | 查看 `web` 规则日志最后 20 行 |
| `portcli logs web -f` | 持续跟踪 `web` 规则日志 |
| `portcli logs --dir` | 输出日志目录路径 |
| `portcli logs web --clear` | 清空 `web` 规则日志 |
| `portcli logs --clear` | 清空守护进程日志 |

## 配置文件和数据目录

### 配置文件

| 平台 | 路径 |
| --- | --- |
| Linux | `~/.config/portcli/config.toml` |
| Windows | `%APPDATA%\portcli\config\config.toml` |

配置文件示例：

```toml
[[rules]]
name = "web"
source = "0.0.0.0:8080"
target = "192.168.31.10:80"
enabled = true

[[rules]]
name = "ssh"
source = "127.0.0.1:2222"
target = "192.168.31.20:22"
enabled = false
```

字段说明：

| 字段 | 说明 |
| --- | --- |
| `name` | 规则名称，必须唯一 |
| `source` | 本机监听地址，格式为 `host:port` |
| `target` | 转发目标地址，格式为 `host:port` |
| `enabled` | 是否由守护进程启动这条规则 |

### 日志和运行时状态

| 平台 | 路径 |
| --- | --- |
| Linux 日志 | `~/.local/share/portcli/logs/` |
| Linux 状态文件 | `~/.local/share/portcli/state.json` |
| Windows 日志 | `%LOCALAPPDATA%\portcli\data\logs\` |
| Windows 状态文件 | `%LOCALAPPDATA%\portcli\data\state.json` |

日志目录结构：

```text
logs/
├── daemon.log
└── rules/
    ├── web.log
    └── ssh.log
```

`state.json` 保存守护进程 PID、本机控制端口和随机 token。守护进程停止时会自动删除该文件。

## 工作机制

```text
CLI (cli.rs)
 │
 ├── 读写配置 ─────────────────► config.toml
 │
 ├── 控制命令 ─────────────────► Daemon (daemon.rs)
 │    (TCP JSON, 127.0.0.1)       │
 │                                ├── Control Server (control.rs)
 │                                ├── Forward Tasks  (forward.rs)
 │                                └── Log Writer     (logs.rs)
 │
 └── 查看日志 ─────────────────► log files
```

1. 规则存储在 TOML 配置文件中，CLI 和守护进程共享同一份配置。
2. 守护进程启动后绑定一个随机的 `127.0.0.1` 控制端口，并把 PID、控制端口和 token 写入 `state.json`。
3. 每条已启用规则会启动一个 TCP listener。
4. 每个入站连接会创建一个异步任务，使用 `tokio::io::copy_bidirectional` 做双向转发。
5. CLI 的 `status`、`stop`、`reload` 通过本机 JSON 控制协议发送给守护进程。
6. 守护进程日志和规则日志分别写入数据目录，便于排障。

## 排障

### 端口被占用

如果规则状态是 `failed`，并且错误类似 `Address already in use`，说明 `--source` 端口已经被其他进程占用。

```bash
portcli status
portcli modify web --source 0.0.0.0:8081
```

### 目标服务连不上

如果规则能启动，但访问转发端口时失败，检查规则日志：

```bash
portcli logs web -n 50
```

如果出现 `connect target failed`，说明守护进程无法连接 `--target` 指向的服务。

### 守护进程状态异常

```bash
portcli status
portcli stop
portcli run
```

如果手动删除或修改了状态文件，也可以直接重新启动守护进程。

## 项目结构

```text
src/
├── main.rs      # 入口点
├── cli.rs       # CLI 参数解析和命令处理
├── config.rs    # TOML 配置读写和地址校验
├── daemon.rs    # 守护进程生命周期、规则管理、优雅退出
├── forward.rs   # TCP 转发逻辑
├── control.rs   # 本机 TCP JSON 控制协议
├── state.rs     # 运行时状态文件
├── logs.rs      # 日志路径、追加、读取、跟踪、清空
└── error.rs     # 错误类型
```

## 当前限制

- 仅支持 TCP，不支持 UDP
- 不提供 TLS 加密
- 未集成 systemd 或 Windows Service
- 没有图形界面
- 手动编辑配置后需要执行 `portcli reload`
- 控制协议只监听本机回环地址，并使用本地状态文件中的随机 token 认证
- 日志跟踪采用 500ms 轮询

## 测试结果

| 平台 | 测试数 | 通过 |
| --- | ---: | ---: |
| Windows 11 (MSVC) | 53 | 53 |
| Ubuntu 24.04 / WSL2 (GNU) | 52 | 52 |

当前仓库保留了 Linux 验证脚本：[test_linux.sh](test_linux.sh) · [test_linux_forward.sh](test_linux_forward.sh)

## 许可证

MIT
