# Release 指南

## Linux 发布：GLIBC 兼容性问题

Rust 在 Ubuntu 上默认编译的二进制动态链接到系统 GLIBC。GitHub Actions 的 `ubuntu-latest`（当前为 Ubuntu 24.04）自带的 GLIBC 版本较新（2.39+），编译出的二进制在旧版服务器（如 CentOS 7 / Debian 10 / Ubuntu 18.04）上运行时会报错：

```
./porthannis: /lib/x86_64-linux-gnu/libc.so.6: version `GLIBC_2.38' not found
```

### 解决方案：musl 静态编译

使用 `x86_64-unknown-linux-musl` target 编译，musl libc 会静态链接进二进制，不依赖系统 GLIBC 版本，几乎所有 Linux 内核 2.6+ 的服务器都能直接运行。

## 构建步骤

### 本地构建 musl 版本

```bash
# 添加 musl target
rustup target add x86_64-unknown-linux-musl

# 安装 musl-gcc（Ubuntu/Debian）
sudo apt install musl-tools

# 构建
cargo build --release --target x86_64-unknown-linux-musl -p porthannis-server

# 产物位于 target/x86_64-unknown-linux-musl/release/porthannis
```

### CI 构建 musl 版本（GitHub Actions）

```yaml
- name: Install musl target
  run: rustup target add x86_64-unknown-linux-musl

- name: Install musl-gcc
  run: sudo apt-get install -y musl-tools

- name: Build CLI (musl)
  run: cargo build --release --target x86_64-unknown-linux-musl -p porthannis-server

- name: Upload artifact
  uses: actions/upload-artifact@v4
  with:
    name: linux-artifact
    path: target/x86_64-unknown-linux-musl/release/porthannis
```

### 验证是否为静态链接

```bash
ldd target/x86_64-unknown-linux-musl/release/porthannis
# 应输出: statically linked
# 而非列出 .so 依赖
```

## 多架构支持

如需同时支持 ARM（树莓派、ARM 云服务器），可额外添加 `aarch64-unknown-linux-musl` target：

```bash
rustup target add aarch64-unknown-linux-musl

# 交叉编译需要 musl 交叉工具链
sudo apt install musl-tools  # x86_64
sudo apt install gcc-aarch64-linux-gnu  # ARM64 交叉编译器
```

## 注意事项

- musl 的堆分配器（malloc）性能略低于 glibc，但对端口转发场景无实际影响
- 如果代码中使用了 `jemalloc` 或 `mimalloc`，需要用 `#[cfg(not(target_env = "musl"))]` 做条件编译
- DNS 解析在 musl 下行为略有不同（musl 严格按 `/etc/hosts` → DNS 顺序查询，不读取 `/etc/nsswitch.conf`）
