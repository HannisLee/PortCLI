# Release Guide

How to publish a new version of portcli.

## Prerequisites

- Rust stable toolchain installed (`rustup update stable`)
- Push access to [HannisLee/PortHannis](https://github.com/HannisLee/PortHannis)
- All checks pass locally

## Step-by-step

### 1. Pre-release checks

```bash
cargo fmt --all --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
cargo build --release --locked
```

All four must pass with zero errors.

### 2. Update version

Edit `Cargo.toml`:

```toml
[package]
version = "0.4.0"   # ← bump this
```

Then regenerate `Cargo.lock`:

```bash
cargo check
```

### 3. Update CHANGELOG.md

Add a new section for the version. Follow the [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) format.

### 4. Update README.md / README_zh.md

If the release changes any command behavior, update the docs and the installation examples (the `wget` / download URLs use the version number).

### 5. Commit

```bash
git add Cargo.toml Cargo.lock CHANGELOG.md README.md README_zh.md
git commit -m "chore: prepare release v0.4.0"
```

### 6. Tag

```bash
git tag -a v0.4.0 -m "Release v0.4.0"
```

Use an annotated tag (`-a`). The tag name must match `v<major>.<minor>.<patch>`.

### 7. Push

```bash
git push origin main
git push origin v0.4.0
```

Pushing the tag triggers the GitHub Actions `Release` workflow.

### 8. Verify

Go to <https://github.com/HannisLee/PortHannis/releases>. You should see:

- A draft release created automatically
- Assets attached:
  - `portcli-v0.4.0-x86_64-unknown-linux-musl.tar.gz`
  - `portcli-v0.4.0-x86_64-pc-windows-msvc.zip`
  - `SHA256SUMS`

Wait for the workflow to finish (≈ 5 minutes), then verify the release.

### 9. Test the release locally

**Linux (musl)**:

```bash
wget https://github.com/HannisLee/PortHannis/releases/download/v0.4.0/portcli-v0.4.0-x86_64-unknown-linux-musl.tar.gz
tar -xzf portcli-v0.4.0-x86_64-unknown-linux-musl.tar.gz
chmod +x portcli
./portcli --help

# Verify static linking
ldd ./portcli
# Expected: "not a dynamic executable" or no glibc dependency
```

**Windows**:

```powershell
Invoke-WebRequest -Uri "https://github.com/HannisLee/PortHannis/releases/download/v0.4.0/portcli-v0.4.0-x86_64-pc-windows-msvc.zip" -OutFile portcli.zip
Expand-Archive portcli.zip -DestinationPath .
.\portcli.exe --help
```

## Troubleshooting

### GLIBC version error on Linux

If a user reports `/lib/x86_64-linux-gnu/libc.so.6: version 'GLIBC_2.xx' not found`, they are using a GNU build. Direct them to the **musl** tarball instead, which is statically linked and has no glibc dependency.

### Release workflow failed

Check the Actions tab on GitHub. Common causes:

- `cargo build --locked` fails — `Cargo.lock` is out of sync with `Cargo.toml`. Run `cargo check` locally and commit the updated lock file.
- Build target not installed — the workflow uses `dtolnay/rust-toolchain@stable` which handles this automatically.

### Tag already exists

Delete the local and remote tags, fix the issue, and re-tag:

```bash
git tag -d v0.4.0
git push origin :refs/tags/v0.4.0
```
