# Building

## Requirements

* Rust 1.95+ (edition 2024). Development used rustc 1.99.1.
* A C toolchain is **not** required by Animara's own crates. Some dependencies' build scripts need a linker, which
  any normal platform toolchain provides (gcc/clang, MSVC, Xcode CLT).
* Linux runtime: Wayland or X11, OpenGL/EGL drivers, `libxkbcommon`. Debian/Ubuntu build packages:
  `libxkbcommon-dev libwayland-dev libxcb-render0-dev libxcb-shape0-dev libxcb-xfixes0-dev`.
* Optional at runtime: **FFmpeg** on `PATH` (video import/export, MP3/OGG/FLAC import). On Linux, an audio
  player: `pw-play`, `paplay`, `aplay` or `ffplay`.

## Commands

```bash
cargo build --release -p animara-app -p animara-cli
cargo run --release -p animara-app -- [files…] [--sample N]
cargo test --workspace
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
```

## Hosts without a C compiler (e.g. SteamOS)

`tools/dev-env.sh` installs Zig (via `uv`) into a scratch directory and uses `zig cc` as the linker. It also moves
`CARGO_HOME` and `CARGO_TARGET_DIR` to `/tmp/animara-build`, so a partition that is short on inodes is not filled:

```bash
source tools/dev-env.sh
cargo build --release -p animara-app
```

The resulting binaries target glibc ≥ 2.34. This is how the project was developed and tested on a Steam Deck.

## Cross-compilation

Release artifacts are built natively on GitHub-hosted runners (see `.github/workflows/release.yml`). Windows ARM64
is cross-compiled on `windows-latest` with the `aarch64-pc-windows-msvc` target. macOS builds both architectures and
merges them with `lipo`.
