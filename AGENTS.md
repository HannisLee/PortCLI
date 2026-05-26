# PortHannis - PortCLI 跨平台端口转发工具

## 项目目标

PortHannis 仓库将重构为纯 CLI 端口转发工具。仓库名继续使用 `PortHannis`，构建产物名称为 `portcli`。

目标平台：
- Linux
- Windows
- macOS

当前重构目标是移除桌面 GUI、Wails、WebUI、系统托盘和前端，只保留命令行管理、后台 daemon 和 TCP 端口转发能力。

详细产品规格以 `spec.md` 为准。版本与发布历史维护以 `version.md` 为准。

## 核心特性

- 单文件可执行程序，无需安装
- `portcli run` 启动后台 daemon
- `portcli status` 查询 daemon 和规则运行状态
- `portcli stop` 关闭后台 daemon 并停止所有转发
- 使用 `port.json` 作为唯一配置文件
- 配置顶层为规则名到规则对象的映射，不使用 ID
- 默认规则名从 `name1` 开始递增
- 简单 TCP 转发：监听地址端口到目标地址端口
- 每条规则独立日志文件，日志路径由 `logPath` 指定或自动生成

## 技术栈

| 组件 | 技术选择 | 说明 |
|------|----------|------|
| 后端语言 | Go 1.22+ | 高性能、跨平台编译、单文件输出 |
| CLI | Go 标准库优先 | 命令少，避免不必要依赖 |
| 后台控制 | 本机 loopback 控制端口 | 跨平台实现 `status` 和 `stop` |
| 配置存储 | JSON 文件 | `port.json` |
| 日志存储 | 循环缓冲文本日志 | 每规则独立文件，10MB 上限 |

## 配置格式

`port.json` 顶层必须是对象，key 是唯一规则名，value 是规则配置。

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

约束：
- 不使用规则 ID。
- 规则名唯一，CLI 默认不展示任何 ID。
- 用户手写 JSON 时可以省略 `logPath` 或留空。
- `portcli add` 或 `portcli enable <name>` 时，如果 `logPath` 为空，自动生成并写回配置。
- 如果用户指定了 `logPath`，程序应尊重该路径。
- JSON 文件不能包含注释。

## CLI 命令

必须支持：

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

命令语义：
- `run`：启动后台 daemon 后返回；如果 daemon 已运行，提示已运行。
- `status`：查询 daemon 状态和每条启用规则的运行状态。
- `stop`：请求 daemon 优雅退出。
- `list`：读取 `port.json`，展示规则名、监听地址、目标地址、启用状态和日志路径。
- `enable`：设置规则启用；必要时自动填充 `logPath`。
- `disable`：设置规则禁用。
- `remove`：删除规则。
- `logs`：读取对应规则日志。
- `clear-logs`：清空对应规则日志。

## 端口策略

- 本地监听端口允许 `1-65535`。
- 不人为限制 `1024-65535`。
- 监听特权端口所需权限由操作系统决定；权限不足时返回清晰错误。
- 目标端口允许 `1-65535`。

## 后台 daemon 设计

推荐实现：
- `portcli run` 派生后台进程，后台进程加载 `port.json` 并启动所有 `enabled=true` 的规则。
- daemon 在 `127.0.0.1` 监听一个控制端口。
- 状态文件保存 daemon 的 `pid`、控制端口和随机 token。
- `status` 和 `stop` 读取状态文件后访问本机控制接口。
- 控制接口必须验证 token。
- daemon 退出时停止所有规则并清理状态文件。

## 开发工作流

1. 从 `tasks.md` 中选择待办任务，标记为进行中。
2. 按 `spec.md` 实现功能。
3. 运行格式化、单元测试和可行的功能测试。
4. 如涉及跨平台行为，至少保证代码在 Linux、Windows、macOS 可编译。
5. 提交信息遵循 Conventional Commits。
6. 完成后更新 `tasks.md`。

## 版本文档维护

- `version.md` 记录 tag、发布摘要和重要历史变更。
- 每次创建新 tag 后，必须更新 `version.md`。
- 每次创建新 tag 后，必须同步更新 `README.md`，确保使用说明、配置格式和发布信息与 tag 对应的行为一致。
- 如果发布行为改变了 `spec.md` 中的产品规格，也必须同步更新 `spec.md`。

## 环境配置

开发工具和缓存文件统一保存在 `D:\Code` 目录：

- Go 安装路径：`D:\Code\Go`
- Go GOPATH：`D:\Code\Go\go-packages`
