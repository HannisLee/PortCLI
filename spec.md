# portcli 项目结构说明

本文档记录当前项目的目录结构、核心模块职责、运行机制和数据流。内容以当前工作目录中的实际文件为准。

## 项目定位

`portcli` 是一个 Rust 编写的跨平台 TCP 端口转发 CLI 工具。用户通过命令行维护转发规则，后台守护进程读取配置并启动对应 TCP listener。CLI 与守护进程之间通过本机 TCP JSON 控制协议通信。

## 顶层目录

```text
.
├── Cargo.toml              # Rust 包声明、版本号和依赖
├── Cargo.lock              # Cargo 锁定文件，记录依赖解析结果
├── README.md               # 中文使用文档和示例
├── LICENSE                 # MIT 许可证
├── spec.md                 # 当前项目结构和核心机制说明
├── version.md              # 版本修改记录
├── AGENTS.md               # 仓库协作规则和代理工作说明
├── test_linux.sh           # Linux CLI 行为测试脚本
├── test_linux_forward.sh   # Linux TCP 转发端到端测试脚本
└── src/
    ├── main.rs             # 程序入口
    ├── cli.rs              # CLI 命令定义和命令处理
    ├── config.rs           # 配置结构、配置读写、规则查找、地址校验
    ├── daemon.rs           # 守护进程生命周期、规则任务管理、控制命令处理
    ├── forward.rs          # TCP 转发 listener 和连接转发逻辑
    ├── control.rs          # 本机 TCP JSON 控制服务端和客户端
    ├── state.rs            # 运行时状态文件读写和守护进程存活检测
    ├── logs.rs             # 日志路径、追加、读取、跟踪、清空
    └── error.rs            # 项目错误类型
```

## Cargo 配置

### 包信息

- 包名：`portcli`
- 当前版本：以 `Cargo.toml` 中 `[package].version` 为准
- Edition：`2021`

### 主要依赖

| 依赖 | 用途 |
| --- | --- |
| `clap` | CLI 参数解析，使用 derive 模式 |
| `tokio` | 异步运行时、TCP listener、异步任务和信号处理 |
| `tokio-util` | `CancellationToken`，用于守护进程和规则任务取消 |
| `serde` / `serde_json` | 配置、状态和控制协议 JSON 序列化 |
| `toml` | 配置文件 TOML 读写 |
| `directories` | 跨平台配置目录和数据目录解析 |
| `anyhow` | 命令处理中的通用错误传播 |
| `thiserror` | 项目自定义错误类型 |
| `tracing` / `tracing-subscriber` | 前台守护进程的结构化日志输出 |
| `chrono` | 文件日志时间戳 |
| `rand` | 守护进程控制 token 生成 |

## 源码模块

### `src/main.rs`

程序入口很薄，只负责：

1. 用 `Cli::parse()` 解析命令行参数。
2. 调用 `cli::handle_command(cli)` 执行命令。
3. 如果返回错误，向 stderr 输出 `error: ...` 并用退出码 `1` 结束。

### `src/cli.rs`

CLI 层定义所有用户可见命令，并把命令映射到内部模块。

#### 命令列表

全局选项：

| 选项 | 作用 |
| --- | --- |
| `--version` / `-V` | 输出当前版本号 |
| `--help` / `-h` | 输出顶层帮助、用途说明和常用示例 |
| `<COMMAND> --help` | 输出子命令详细说明和示例 |

| 命令 | 作用 |
| --- | --- |
| `list` | 列出所有规则，守护进程运行时尽量显示运行状态 |
| `add <name> --source <addr> --target <addr>` | 添加规则，新规则默认禁用 |
| `modify <name> [--source <addr>] [--target <addr>]` | 修改规则源地址或目标地址 |
| `remove <name>` | 删除规则 |
| `enable <name>` | 启用规则 |
| `disable <name>` | 禁用规则 |
| `run [--foreground]` | 启动守护进程，默认后台运行 |
| `status` | 查看守护进程和规则运行状态 |
| `stop` | 停止守护进程 |
| `reload` | 通知守护进程重新加载配置 |
| `logs [name]` | 查看、跟踪、清空日志或输出日志目录 |

#### 行为细节

- `add` 会校验 `source` 和 `target` 是否是 `host:port` 格式，且端口必须能解析为 `u16`。
- `add` 创建的规则 `enabled = false`。
- `modify` 不允许不传 `--source` 和 `--target`。
- `modify`、`remove`、`enable`、`disable` 在守护进程运行时会自动发送 `reload` 控制命令。
- `status` 在守护进程未运行时会显示静态配置；守护进程运行时会通过控制协议获取实时状态。
- `logs --dir` 只输出日志目录，不读取日志文件。
- `logs --clear` 会清空对应日志文件。
- 顶层 `--help` 展示版本、工具用途和常用示例。
- 常用子命令通过 `long_about` 和 `after_help` 提供更详细说明和小样例。

### `src/config.rs`

负责配置数据模型、配置文件路径、读写和规则查找。

#### 数据结构

```rust
pub struct Rule {
    pub name: String,
    pub source: String,
    pub target: String,
    pub enabled: bool,
}

pub struct Config {
    pub rules: Vec<Rule>,
}
```

#### 配置路径

通过 `directories::ProjectDirs::from("", "", "portcli")` 获取跨平台路径。

| 平台 | 配置文件 |
| --- | --- |
| Linux | `~/.config/portcli/config.toml` |
| Windows | `%APPDATA%\portcli\config\config.toml` |

#### 配置格式

```toml
[[rules]]
name = "web"
source = "0.0.0.0:8080"
target = "127.0.0.1:3000"
enabled = true
```

### `src/daemon.rs`

守护进程核心模块，负责启动、停止、控制服务、规则任务管理和重载。

#### 启动模式

- `portcli run`：通过 `std::process::Command` 重新执行当前二进制，并传入 `run --foreground`，父进程立即返回。
- `portcli run --foreground`：在当前进程中创建 tokio runtime 并运行守护进程主循环。
- Windows 后台启动使用 `DETACHED_PROCESS`。

#### 守护进程启动流程

1. 读取配置文件。
2. 在 `127.0.0.1:0` 绑定随机控制端口。
3. 生成 32 位随机控制 token。
4. 写入运行时状态文件。
5. 创建日志目录并写入 daemon 启动日志。
6. 启动控制服务。
7. 为每条 `enabled = true` 的规则启动转发任务。
8. 进入主循环，等待控制命令、取消信号或 Ctrl+C。

#### 规则任务管理

内部使用 `HashMap<String, ManagedRule>` 维护已启动规则：

- `cancel_token`：取消单条规则任务。
- `join_handle`：等待规则任务退出。
- `status_rx`：读取规则状态。
- `rule`：规则配置副本。

#### 控制命令

| 命令 | 行为 |
| --- | --- |
| `status` | 返回 PID、控制地址和所有运行中规则状态 |
| `stop` | 返回停止消息，并触发根取消 token |
| `reload` | 停止全部规则任务，重新读取配置，只启动已启用规则 |

### `src/forward.rs`

负责实际 TCP 转发。

#### 规则状态

```rust
pub enum RuleStatus {
    Starting,
    Running,
    Stopped,
    Failed(String),
}
```

#### 转发流程

1. 在规则 `source` 地址上绑定 `TcpListener`。
2. 绑定成功后写入规则启动日志，并把状态设为 `Running`。
3. 每次接收一个入站连接后创建独立 tokio 任务。
4. 连接任务连接 `target`。
5. 使用 `tokio::io::copy_bidirectional` 做双向转发。
6. 连接结束后写入 `bytes_sent` 和 `bytes_received`。
7. 如果绑定失败，规则状态变为 `Failed(error)`。

### `src/control.rs`

负责 CLI 与守护进程之间的本机 JSON 控制协议。

#### 协议载体

- 传输层：TCP
- 监听地址：`127.0.0.1:<随机端口>`
- 消息格式：单行 JSON，以 `\n` 结尾
- 认证：请求必须携带运行时状态文件中的随机 token

#### 请求示例

```json
{"token":"...","command":"status"}
```

#### 响应示例

```json
{
  "ok": true,
  "pid": 12345,
  "control": "127.0.0.1:45678",
  "rules": [
    {
      "name": "web",
      "source": "0.0.0.0:8080",
      "target": "127.0.0.1:3000",
      "enabled": true,
      "status": "running",
      "error": null
    }
  ]
}
```

### `src/state.rs`

负责运行时状态文件。

#### 数据结构

```rust
pub struct RuntimeState {
    pub pid: u32,
    pub control_host: String,
    pub control_port: u16,
    pub token: String,
}
```

#### 状态路径

| 平台 | 状态文件 |
| --- | --- |
| Linux | `~/.local/share/portcli/state.json` |
| Windows | `%LOCALAPPDATA%\portcli\data\state.json` |

#### 存活检测

`is_daemon_running()` 会读取状态文件，并尝试在 500ms 内连接控制端口。连接成功表示守护进程运行中。

### `src/logs.rs`

负责文件日志。

#### 日志路径

| 平台 | 日志目录 |
| --- | --- |
| Linux | `~/.local/share/portcli/logs/` |
| Windows | `%LOCALAPPDATA%\portcli\data\logs\` |

#### 日志结构

```text
logs/
├── daemon.log
└── rules/
    └── <rule-name>.log
```

#### 日志格式

```text
[YYYY-MM-DD HH:MM:SS] LEVEL message
```

#### 支持操作

- 追加日志。
- 读取最后 N 行。
- 清空日志。
- 轮询跟踪日志，间隔 500ms。

### `src/error.rs`

集中定义项目错误类型，包括：

- 规则已存在或不存在。
- 地址格式不合法。
- 修改命令没有指定变更字段。
- 日志文件不存在。
- 配置读写失败。
- 状态文件读写失败。
- 端口绑定失败。
- 控制协议错误。
- IO 错误。

## 核心运行数据流

### 添加并启用规则

```text
用户命令
  └─ portcli add web --source 0.0.0.0:8080 --target 127.0.0.1:3000
       ├─ cli.rs 校验地址
       ├─ config.rs 读取配置
       ├─ 检查规则名唯一
       └─ config.rs 写回 config.toml，enabled=false

用户命令
  └─ portcli enable web
       ├─ config.rs 设置 enabled=true
       ├─ config.rs 写回 config.toml
       └─ 如果守护进程运行，通过 control.rs 发送 reload
```

### 启动守护进程

```text
portcli run
  └─ daemon.rs 后台拉起当前二进制 run --foreground
       └─ run_daemon_async()
            ├─ 读取 config.toml
            ├─ 绑定 127.0.0.1:0 控制端口
            ├─ 生成 token
            ├─ 写入 state.json
            ├─ 启动控制服务
            └─ 为 enabled=true 的规则启动 forward task
```

### 处理连接

```text
客户端连接 source
  └─ forward.rs listener.accept()
       ├─ 创建连接任务
       ├─ TcpStream::connect(target)
       ├─ copy_bidirectional(inbound, outbound)
       └─ 写入连接关闭和字节数日志
```

### 重载配置

```text
portcli reload
  └─ control.rs 读取 state.json 并发送 JSON
       └─ daemon.rs 收到 reload
            ├─ 停止全部当前规则任务
            ├─ 重新读取 config.toml
            └─ 启动所有 enabled=true 的规则
```

## 测试脚本

### `test_linux.sh`

覆盖 Linux 下的 CLI 行为测试，包括：

- `--version` 和 `--help`
- 空配置 `list`
- 规则新增、重复新增、修改、删除
- 地址校验
- 启用和禁用
- 守护进程启动、重复启动、状态、停止、重载
- 日志读取、行数限制、清空、目录输出
- 状态文件清理
- 前台守护进程

### `test_linux_forward.sh`

覆盖 Linux 下端到端 TCP 转发：

- 配置测试规则。
- 启动守护进程。
- 用 `nc` 启动后端监听。
- 通过转发端口发送数据。
- 查看规则日志确认连接经过转发。

## 当前限制

- 仅支持 TCP，不支持 UDP。
- 不内置 TLS。
- 不提供 systemd 或 Windows Service 集成。
- 不提供 GUI。
- 手动编辑配置文件后需要执行 `portcli reload`。
- 控制协议仅监听本机回环地址，认证依赖状态文件中的随机 token。
- 日志跟踪采用轮询实现。
