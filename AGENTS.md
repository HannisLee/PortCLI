# AGENTS.md

本文件只保留核心协作规则。项目结构、架构和模块职责请查 `spec.md`；版本细节和历史修改请查 `version.md`。

## 必读入口

- 架构、目录、模块职责、运行机制：`spec.md`
- 版本记录、每版修改细节：`version.md`
- 用户使用说明：`README.md`

## 语言规则

- 回答尽量使用中文。
- 文档新增或修改优先使用中文。
- commit message 必须使用中文。
- 变更摘要、测试说明、PR 描述优先使用中文。

## 版本规则

- 每次任务只要涉及文件修改，必须至少递增补丁版本号。
- 最小递增示例：`0.0.0 -> 0.0.1`、`0.4.1 -> 0.4.2`。
- 中版本号和大版本号由用户手动指定，不要擅自提升。
- 修改版本时必须同步：
  - `Cargo.toml`
  - `Cargo.lock`
  - `version.md`

## 修改规则

- 修改前先看 `git status --short`。
- 不要恢复、还原、删除或覆盖用户已有改动，除非用户明确要求。
- 只改当前任务相关文件。
- 手动编辑优先使用补丁方式。
- 不要为文档任务改代码逻辑。
- 修改 CLI 行为时，同步更新 `README.md`、`spec.md`、`version.md`。
- 修改项目结构时，同步更新 `spec.md` 和本文件。

## 验证规则

- 文件修改后至少运行：

```bash
cargo test
```

- 涉及 Linux 转发行为时，再按需运行：

```bash
./test_linux.sh
./test_linux_forward.sh
```

注意：测试脚本可能访问或清理用户 home 下的 `~/.config/portcli` 和 `~/.local/share/portcli`，执行前必须确认影响。
