#!/usr/bin/env bash
set -euo pipefail

workspace=/workspaces/zephyrproject
revision=072fd9442fe12568bcc3d3c61e4f390ed5cf35a3
venv="$workspace/.venv"
sdk="$workspace/.sdk"

if [[ ! -d "$workspace" ]]; then
    if [[ -w /workspaces ]]; then
        mkdir -p "$workspace"
    else
        sudo -n install -d -o "$(id -u)" -g "$(id -g)" "$workspace"
    fi
fi

python3 -m venv "$venv"
"$venv/bin/python" -m pip install --upgrade pip west
export PATH="$venv/bin:$PATH"

if [[ ! -f "$workspace/.west/config" ]]; then
    if [[ -e "$workspace/zephyr" ]]; then
        printf 'Existing Zephyr checkout is not in an initialized west workspace.\n' >&2
        exit 1
    fi
    git init "$workspace/zephyr"
    git -C "$workspace/zephyr" fetch --depth 1 https://github.com/zephyrproject-rtos/zephyr "$revision"
    git -C "$workspace/zephyr" checkout --detach FETCH_HEAD
    west init -l "$workspace/zephyr"
fi

cd "$workspace"
if [[ "$(git -C zephyr rev-parse HEAD)" != "$revision" ]]; then
    printf 'Existing Zephyr checkout does not match %s; refusing to modify it.\n' "$revision" >&2
    exit 1
fi

project_filter="$(west config manifest.project-filter || true)"
case ",$project_filter," in
    *,+zephyr-lang-rust,*) ;;
    *) west config manifest.project-filter "${project_filter:+$project_filter,}+zephyr-lang-rust" ;;
esac
west update
"$venv/bin/python" -m pip install -r zephyr/scripts/requirements.txt
west packages pip --install
west zephyr-export

case "$(uname -m)" in
    x86_64) rustup target add x86_64-unknown-none ;;
    aarch64) rustup target add aarch64-unknown-none ;;
esac
rustup target add riscv32imafc-unknown-none-elf

if [[ ! -x "$sdk/gnu/riscv64-zephyr-elf/bin/riscv64-zephyr-elf-gcc" ]]; then
    west sdk install --install-dir "$sdk" --gnu-toolchains riscv64-zephyr-elf
fi