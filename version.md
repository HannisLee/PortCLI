# 版本记录

本文档记录 `portcli` 各版本的修改细节。版本号以 `Cargo.toml` 为准；未来涉及文件修改时，至少递增补丁版本号，例如 `0.4.3 -> 0.4.4`。中版本号和大版本号由项目维护者手动指定。

## 0.4.3

本版本为 CLI 帮助信息增强版本。

### CLI

- 明确保留并展示全局 `--version` / `-V` 版本选项。
- 完善顶层 `--help`，增加更详细的工具用途说明。
- 顶层 `--help` 增加常用示例：
  - 查看版本。
  - 添加规则。
  - 启用规则。
  - 启动守护进程。
  - 查看状态。
  - 查看规则日志。
- 为 `list`、`add`、`modify`、`remove`、`enable`、`disable`、`run`、`status`、`stop`、`reload`、`logs` 增加更详细的命令说明。
- 为常用子命令增加命令级示例，可通过 `portcli <COMMAND> --help` 查看。

### 文档

- 更新 README 命令参考，补充全局 `--version` / `--help` 说明。
- 更新 `spec.md`，记录 CLI 全局选项和帮助信息策略。

### 版本

- 将 `Cargo.toml` 中的版本从 `0.4.2` 提升到 `0.4.3`。
- 同步更新 `Cargo.lock` 中 `portcli` 包版本。

## 0.4.2

本版本为协作规则文档精简版本。

### 文档

- 精简 `AGENTS.md`，只保留核心协作规则。
- 将架构、目录、模块职责和运行机制说明统一指向 `spec.md`。
- 将版本细节和历史修改说明统一指向 `version.md`。

### 版本

- 将 `Cargo.toml` 中的版本从 `0.4.1` 提升到 `0.4.2`。
- 同步更新 `Cargo.lock` 中 `portcli` 包版本。

## 0.4.1

本版本为文档和协作规范版本。

### 文档

- 将主 README 更新为中文文档。
- 在 README 中补充更多真实使用样例：
  - 本机端口转发快速开始。
  - 将本机开发服务暴露给局域网。
  - SSH 端口转发。
  - 多规则管理。
  - 前台调试。
  - 日志查看、跟踪、清空。
  - 手动编辑配置后的重载。
  - Linux / WSL 端到端连通性测试。
- 新增 `spec.md`，记录项目结构、核心模块职责、运行数据流、控制协议、日志路径和测试脚本。
- 新增 `version.md`，用于持续记录版本修改细节。
- 新增 `AGENTS.md`，记录后续协作和代码修改规则。
- 将 README 底部的测试报告链接调整为当前仍存在的 Linux 测试脚本链接。

### 版本

- 将 `Cargo.toml` 中的版本从 `0.4.0` 提升到 `0.4.1`。
- 同步更新 `Cargo.lock` 中 `portcli` 包版本。

## 0.4.0

当前代码和 README 安装示例显示项目曾发布 `0.4.0`。可确认能力如下。

### CLI 命令

- 提供 `list`、`add`、`modify`、`remove`、`enable`、`disable`、`run`、`status`、`stop`、`reload`、`logs` 命令。
- 新增规则默认禁用，需要执行 `enable` 后才会由守护进程启动。
- 修改、删除、启用、禁用规则时，如果守护进程正在运行，会自动发送 `reload`。
- `logs` 支持：
  - 读取 daemon 日志。
  - 读取指定规则日志。
  - `-n/--lines` 指定读取行数。
  - `-f/--follow` 跟踪日志。
  - `--clear` 清空日志。
  - `--dir` 输出日志目录。

### 配置和状态

- 使用 TOML 配置文件保存转发规则。
- 规则结构包含 `name`、`source`、`target`、`enabled`。
- 使用 `directories` crate 遵循 Linux XDG 和 Windows AppData 路径规范。
- 守护进程运行时写入 `state.json`，保存 PID、控制地址、控制端口和随机 token。
- 守护进程停止后删除运行时状态文件。

### 守护进程

- `portcli run` 默认后台启动守护进程。
- `portcli run --foreground` 支持前台运行，便于调试。
- 守护进程启动后绑定随机本机控制端口。
- 守护进程会为所有 `enabled = true` 的规则启动转发任务。
- 重复执行 `portcli run` 时会检测已有守护进程，避免重复启动。
- 支持 `status`、`stop`、`reload` 控制命令。

### TCP 转发

- 每条启用规则在 `source` 上绑定 TCP listener。
- 每个入站连接创建独立异步任务。
- 使用 `tokio::io::copy_bidirectional` 实现双向转发。
- 规则状态包含 `starting`、`running`、`stopped`、`failed`。
- 端口绑定失败会在状态中显示错误。
- 目标连接失败会写入规则日志。

### 日志

- daemon 日志和规则日志分开记录。
- 日志格式为 `[YYYY-MM-DD HH:MM:SS] LEVEL message`。
- 规则日志记录：
  - 规则启动。
  - 入站连接建立。
  - 目标连接失败。
  - 转发错误。
  - 连接关闭和字节数。

### 已知限制

- 仅支持 TCP。
- 不支持 UDP。
- 不内置 TLS。
- 未集成 systemd 或 Windows Service。
- 没有 GUI。
- 手动编辑配置文件后需要执行 `portcli reload`。
- 日志跟踪采用 500ms 轮询。

## 历史版本说明

当前目录中已经没有独立的历史 changelog 文件，因此 `0.4.0` 之前的版本细节无法从当前文件可靠还原。后续版本请持续在本文件中追加记录。
