#!/usr/bin/env bash
set -euo pipefail

workspace="$(cd "$(dirname "$0")/.." && pwd)"
venv="$workspace/.venv"
sdk="${ZEPHYR_SDK_INSTALL_DIR:-$workspace/.sdk}"

cd "$workspace"
git submodule update --init
git -C components/micropython submodule update --init lib/micropython-lib
python3 -m venv "$venv"
"$venv/bin/python" -m pip install --upgrade pip west
export PATH="$venv/bin:$PATH"
export ZEPHYR_BASE="$workspace/zephyr"

if [[ ! -f "$workspace/.west/config" ]]; then
    west init -l zephyr
fi

if [[ "$(west topdir)" != "$workspace" || "$(west config manifest.path)" != "zephyr" ]]; then
    printf 'Existing west workspace does not use this project and its Zephyr submodule.\n' >&2
    exit 1
fi

project_filter="$(west config manifest.project-filter 2>/dev/null || true)"
case ",$project_filter," in
    *,+zephyr-lang-rust,*) ;;
    *) west config manifest.project-filter "${project_filter:+$project_filter,}+zephyr-lang-rust" ;;
esac
west update --narrow --fetch-opt=--depth=1 zephyr-lang-rust hal_espressif
"$venv/bin/python" -m pip install -r zephyr/scripts/requirements-base.txt \
    -r modules/hal/espressif/zephyr/requirements.txt

case "$(uname -m)" in
    x86_64) rustup target add x86_64-unknown-none ;;
    aarch64) rustup target add aarch64-unknown-none ;;
esac
rustup target add riscv32imafc-unknown-none-elf

if [[ ! -x "$sdk/gnu/riscv64-zephyr-elf/bin/riscv64-zephyr-elf-gcc" ]]; then
    west sdk install --install-dir "$sdk" --gnu-toolchains riscv64-zephyr-elf
fi