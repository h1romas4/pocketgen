#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$root"

python3 tests/maze_demo_test.py
python3 tests/starfield_demo_test.py
python3 tests/cube_demo_test.py
python3 tests/mexican_hat_demo_test.py
python3 tests/mexican_hat_graphics_demo_test.py
python3 tests/plot_test.py
python3 tests/chart_test.py
python3 tests/numeric_test.py
cargo test --manifest-path components/pocketgen-editor/Cargo.toml \
    --target-dir build/editor-tests --test syntax

west build -b native_sim/native/64 . -d build/native_selftest -- \
    -DEXTRA_CONF_FILE=tests/selftest.conf \
    -DDTC_OVERLAY_FILE=boards/native_sim_native_64.overlay

output="$(mktemp)"
trap 'rm -f "$output"' EXIT
status=0
SDL_VIDEODRIVER=dummy timeout 20s build/native_selftest/zephyr/zephyr.exe >"$output" 2>&1 || status=$?
cat "$output"
if [[ "$status" -ne 124 ]]; then
    printf 'Simulator exited unexpectedly: %s\n' "$status" >&2
    exit 1
fi
for message in \
    'MicroPython integration tests passed' \
    'Graphics integration test passed' \
    'Terminal bridge test passed' \
    'Editor viewport test passed' \
    'Search test passed' \
    'Syntax highlighting test passed' \
    'Pane divider test passed' \
    'Solid blue maze wall test passed' \
    'Selection and menu test passed' \
    'Demo loading test passed' \
    'Starfield execution test passed' \
    'Cube execution test passed' \
    'Mexican hat execution test passed' \
    'Terminal controls test passed' \
    'MicroPython heap status test passed' \
    'Startup editor display test passed' \
    'Menu and shortcut bar test passed' \
    'MicroPython execution completed' \
    'Editor buffer execution test passed' \
    'Rendered two terminals on a 1024x600 display' \
    'Editor input ready on virtual UART'; do
    if ! grep -Fq "$message" "$output"; then
        printf 'Missing expected output: %s\n' "$message" >&2
        exit 1
    fi

done
if grep -Fq '<err>' "$output"; then
    printf 'Simulator reported an error\n' >&2
    exit 1
fi

python3 tests/uart-integration.py build/native_selftest/zephyr/zephyr.exe
python3 tests/native-launcher.py build/native_selftest/zephyr/zephyr.exe
if command -v screen >/dev/null; then
    python3 tests/uart-integration.py build/native_selftest/zephyr/zephyr.exe --screen
fi
