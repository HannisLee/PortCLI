# CLAUDE.md

PortHannis — 轻量级端口转发管理器，Rust 核心 + CLI + 内嵌 WebUI。

## 项目结构

```
PortHannis/
├── Cargo.toml               # Rust workspace
├── server/
│   ├── core.rs              # TCP 转发核心（单文件，所有核心逻辑）
│   ├── web.html             # 内嵌 WebUI（单 HTML 文件）
│   ├── src/main.rs          # CLI 入口 + HTTP API 服务器
│   └── Cargo.toml
├── gui/                     # Tauri 桌面应用
│   ├── src/main.rs          # Tauri 入口
│   └── tauri.conf.json
└── README.md
```

## 配置文件

`port.json` 位于用户 home 目录：
- Windows: `C:\Users\<用户名>\port.json`
- Linux/macOS: `~/port.json`

首次运行自动创建。

## CLI 命令

```
porthannis list [-v, --verbose]           # 列出条目（-v 显示全部字段）
porthannis add -n <名称> -s <源端口> [-a <目标地址>] -t <目标端口> [--source-address <地址>]
porthannis modify <ID> [--name/--source-port/--source-address/--target-address/--target-port/--enabled]
porthannis delete <ID>                    # 安全删除（自动停止转发）
porthannis start <ID>                     # 启动转发
porthannis stop <ID>                      # 停止转发
porthannis serve [-p <端口>] [--host <地址>] [--no-open]
```

## 核心文件说明

### server/core.rs (~900 行)
- **数据结构**：`ForwardingEntry`, `EntryStatus`, `EntryRequest`, `PartialUpdate`, `LogMessage`
- **TCP 转发**：`TcpProxy` - 基于 tokio 的异步 TCP 双向转发
- **配置管理**：`ConfigStore` - 读写 `port.json`（路径通过 `dirs::home_dir()`）
- **日志轮转**：`EntryLogger` - 1MB × 5 文件轮转
- **生命周期**：`ProxyManager` - 启动/停止/查询转发条目
- **API 处理函数**：所有 HTTP 端点的实现

### server/src/main.rs
CLI 分发入口 + HTTP API 服务器：
- clap 命令行解析（list/add/modify/delete/start/stop/serve）
- list/add/modify 为同步函数，直接操作 ConfigStore
- delete/start/stop 为异步函数（`#[tokio::main]`），通过 ProxyManager 操作
- serve 启动 Axum HTTP 服务器 + 内嵌 WebUI

## API 端点

| 方法 | 端点 | 描述 |
|------|------|------|
| GET | `/api/health` | 健康检查 |
| GET | `/api/entries` | 列出所有条目 |
| POST | `/api/entries` | 创建新条目 |
| GET | `/api/entries/{id}` | 获取单个条目 |
| PUT | `/api/entries/{id}` | 更新条目 |
| DELETE | `/api/entries/{id}` | 删除条目 |
| POST | `/api/entries/{id}/start` | 启动转发 |
| POST | `/api/entries/{id}/stop` | 停止转发 |
| GET | `/api/entries/{id}/status` | 查询状态 |
| GET | `/api/entries/{id}/logs` | 获取日志 |

## port.json 格式

```json
{
  "entries": [
    {
      "id": "uuid-v4",
      "name": "示例转发",
      "source_address": "0.0.0.0",
      "source_port": 8080,
      "target_address": "192.168.3.11",
      "target_port": 80,
      "enabled": true,
      "log_directory": "logs/example",
      "created_at": "2026-01-01T00:00:00Z",
      "updated_at": "2026-01-01T00:00:00Z"
    }
  ]
}
```

## 技术栈

| 组件 | 技术 |
|------|------|
| CLI | clap 4 (derive) |
| 表格输出 | comfy-table 7 |
| 目录路径 | dirs 6 |
| HTTP API | Axum 0.8 |
| TCP 转发 | tokio + tokio-util |
| 序列化 | serde + serde_json |
| 日志 | tracing |
| 桌面应用 | Tauri 2 |

## 快速开始

```bash
# 开发运行
cargo run -p porthannis-server -- list

# 构建发布版本
cargo build --release -p porthannis-server
# 二进制: target/release/porthannis (或 .exe)
```

> **Linux 发布注意事项**：默认构建产物动态链接 GLIBC，在旧版服务器上会报 `GLIBC_2.xx not found` 错误。发布时请使用 musl 静态编译，详见 [release.md](release.md)。

## 日志系统

每个转发条目都有独立的日志目录：
- `logs/{entry_name}/current.log` - 当前日志
- 单个文件最大 1MB，最多保留 5 个历史文件

## 开发状态

- ✅ **server/core.rs** - TCP 转发核心
- ✅ **server/src/main.rs** - CLI 命令 + HTTP API
- ✅ **port.json** - 配置管理（用户 home 目录）
- ⏳ **gui/** - Tauri GUI
