# PortHannis / PortCLI 任务列表

## PortCLI 重构

- [x] 更新 `spec.md`，确认 PortCLI 产品规格
- [x] 更新 `AGENTS.md`、`CLAUDE.md`、`README.md`、`version.md`
- [x] 将配置格式改为 `port.json` 顶层规则名映射
- [x] 删除规则 ID，改用规则名作为唯一标识
- [x] 实现 `portcli add/list/enable/disable/remove`
- [x] 实现 `portcli run/status/stop` 后台 daemon 控制
- [x] 实现 `portcli logs/clear-logs`
- [x] 改造 TCP forwarder 为规则名模型
- [x] 删除 Wails、WebUI、frontend、tray 和旧构建资源
- [x] 移除不再需要的 Go 依赖
- [x] 运行 `go fmt ./...`
- [x] 运行 `go test ./...`
- [x] 运行 `go vet ./...`
- [x] 验证 Windows 本地产物构建
- [x] 验证 Linux/macOS amd64 交叉编译

## 后续候选任务

- [ ] 增加单元测试覆盖配置管理、地址解析和 daemon 状态文件逻辑
- [ ] 增加 GitHub Actions 跨平台 release 构建
- [ ] 在真实 Linux/macOS 环境做 daemon 端到端测试
