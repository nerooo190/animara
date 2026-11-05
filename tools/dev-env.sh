#!/bin/bash
# Development environment for hosts WITHOUT a system C compiler/linker (e.g. SteamOS).
# Keeps the toolchain, cargo cache and build output in a scratch directory (default /tmp,
# which has plenty of inodes) and uses `zig cc` as the C compiler and linker.
#
#   source tools/dev-env.sh
#
# On normal Linux/macOS/Windows hosts with gcc/clang/MSVC installed, you do NOT need this.
ANIMARA_BUILD_ROOT="${ANIMARA_BUILD_ROOT:-/tmp/animara-build}"
T="$ANIMARA_BUILD_ROOT/toolchain"
mkdir -p "$T/bin"
if [ ! -x "$T/py/ziglang/zig" ]; then
  echo "Installing zig (C toolchain) into $T ..."
  UV_CACHE_DIR="$ANIMARA_BUILD_ROOT/uv-cache" uv pip install --quiet --target "$T/py" ziglang || return 1
fi
for tool in zcc zcxx; do
  mode=cc; [ "$tool" = zcxx ] && mode=c++
  cat > "$T/bin/$tool" <<SH
#!/bin/bash
args=()
for a in "\$@"; do
  case "\$a" in
    -lgcc_s) args+=("-lunwind") ;;
    -Wl,-znoexecstack|--target=*) ;;
    *) args+=("\$a") ;;
  esac
done
exec "$T/py/ziglang/zig" $mode -target x86_64-linux-gnu.2.34 "\${args[@]}"
SH
done
printf '#!/bin/bash\nexec "%s/py/ziglang/zig" ar "$@"\n' "$T" > "$T/bin/zar"
chmod +x "$T/bin/"*
export CC="$T/bin/zcc" CXX="$T/bin/zcxx" AR="$T/bin/zar"
export CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER="$T/bin/zcc"
export CARGO_HOME="$ANIMARA_BUILD_ROOT/cargo-home"
export CARGO_TARGET_DIR="$ANIMARA_BUILD_ROOT/target"
export CARGO_INCREMENTAL=0
export PATH="$HOME/.cargo/bin:$PATH"
