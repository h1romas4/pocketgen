# pocketgen

pocketgen is a pocket computer application for Zephyr RTOS. It combines an
editor derived from Microsoft Edit with MicroPython: edit Python source in the
left pane and run it in the right pane. It supports native simulation and the
ESP32-P4 Function EV Board.

![pocketgen running the 3D Cube demo in native-sim](docs/images/native-sim-cube.png)

## Zephyr setup

These steps use Ubuntu 26.04 on x86-64 and create a separate Zephyr workspace.
Zephyr is pinned to commit
[`072fd9442fe12568bcc3d3c61e4f390ed5cf35a3`](https://github.com/zephyrproject-rtos/zephyr/commit/072fd9442fe12568bcc3d3c61e4f390ed5cf35a3)
(`4.5.99`, a development revision, not the 4.4 stable release).

Install the system dependencies and [Rust with rustup](https://rustup.rs/):

```sh
sudo apt update
sudo apt install -y ca-certificates ccache cmake device-tree-compiler dfu-util \
    file g++-multilib gcc-multilib git gperf libclang-dev libmagic1 libsdl2-dev \
    make ninja-build python3-dev python3-pip python3-tk python3-venv screen wget xz-utils
```

Clone pocketgen, including its MicroPython submodule:

```sh
git clone --recurse-submodules https://github.com/h1romas4/pocketgen.git
cd pocketgen
export POCKETGEN_DIR="$PWD"
```

Create a **new, empty** Zephyr workspace. If this path already exists, choose a
different unused path instead of modifying an existing workspace. The downloads
may take some time.

```sh
export POCKETGEN_ZEPHYR_WORKSPACE="$HOME/.local/share/pocketgen/zephyrproject"
mkdir -p "$POCKETGEN_ZEPHYR_WORKSPACE"
python3 -m venv "$POCKETGEN_ZEPHYR_WORKSPACE/.venv"
source "$POCKETGEN_ZEPHYR_WORKSPACE/.venv/bin/activate"
python -m pip install --upgrade pip west

git init "$POCKETGEN_ZEPHYR_WORKSPACE/zephyr"
git -C "$POCKETGEN_ZEPHYR_WORKSPACE/zephyr" fetch --depth 1 \
    https://github.com/zephyrproject-rtos/zephyr \
    072fd9442fe12568bcc3d3c61e4f390ed5cf35a3
git -C "$POCKETGEN_ZEPHYR_WORKSPACE/zephyr" checkout --detach FETCH_HEAD
west init -l "$POCKETGEN_ZEPHYR_WORKSPACE/zephyr"
cd "$POCKETGEN_ZEPHYR_WORKSPACE"
west config manifest.project-filter +zephyr-lang-rust
west update
python -m pip install -r zephyr/scripts/requirements.txt
west packages pip --install
west zephyr-export

rustup target add x86_64-unknown-none riscv32imafc-unknown-none-elf
west sdk install --install-dir "$POCKETGEN_ZEPHYR_WORKSPACE/.sdk" \
    --gnu-toolchains riscv64-zephyr-elf
export ZEPHYR_BASE="$POCKETGEN_ZEPHYR_WORKSPACE/zephyr"
export ZEPHYR_SDK_INSTALL_DIR="$POCKETGEN_ZEPHYR_WORKSPACE/.sdk"
cd "$POCKETGEN_DIR"
```

In each new shell, select the same workspace before building:

```sh
export POCKETGEN_ZEPHYR_WORKSPACE="$HOME/.local/share/pocketgen/zephyrproject"
source "$POCKETGEN_ZEPHYR_WORKSPACE/.venv/bin/activate"
export ZEPHYR_BASE="$POCKETGEN_ZEPHYR_WORKSPACE/zephyr"
export ZEPHYR_SDK_INSTALL_DIR="$POCKETGEN_ZEPHYR_WORKSPACE/.sdk"
```

GitHub Actions builds native-sim and ESP32-P4 and runs the native integration
and component tests on Ubuntu 26.04.

## Build

Build and run the native simulator from the repository root:

```sh
west build -b native_sim/native/64 . -d build/native_display
python3 scripts/native-sim.py
```

The SDL window is for display only. After it opens, focus the terminal running
the launcher to enter keys; typing while the SDL window is focused does not
send input to pocketgen.

Press **Ctrl+R** to run the editor buffer, **Ctrl+C** to interrupt execution,
and **Ctrl+]** to exit the simulator. The **F10** menu provides sample demos.

## ESP32-P4 setup

![pocketgen running the maze demo on the ESP32-P4 Function EV Board](docs/images/native-sim-cube.jpg)

The configuration targets the ESP32-P4 Function EV Board with a 1024x600
EK79007 DSI LCD and PSRAM. It requires the FPU and the Rust
`riscv32imafc-unknown-none-elf` target. With both the board and LCD power
disconnected, connect the LCD adapter:

| LCD adapter | ESP32-P4 Function EV Board |
| --- | --- |
| J3 | MIPI DSI connector |
| J6 `RST_LCD` | J1 GPIO27 |
| J6 `PWM` | J1 GPIO26 |

Use a reverse-direction LCD ribbon cable, with contacts on opposite sides at
its ends. When powering the LCD adapter through its J1 USB connector, do not
also connect its 5V and GND pins to the board. See Espressif's
[hardware setup guide](https://docs.espressif.com/projects/esp-dev-kits/en/latest/esp32p4/esp32-p4-function-ev-board/user_guide_v1.4.html#hardware-setup).

Connect the board's USB-UART port and build and flash from the repository root:

```sh
west build -b esp32p4_function_ev_board/esp32p4/hpcore . -d build/esp32p4_pocketgen
west flash -d build/esp32p4_pocketgen --esp-device /dev/ttyUSB0
```

Replace `/dev/ttyUSB0` with the board's serial device. After flashing, connect
to UART0 at 115200 baud:

```sh
screen /dev/ttyUSB0 115200
```

Type in this terminal while watching the LCD. The board configuration starts
the 3D Cube demo automatically; **Ctrl+C** stops it and returns to the editor.
Use **Ctrl+R** to run the buffer and **Ctrl+E** to open the menu. `screen`
reserves Ctrl+A: to send Ctrl+A to the editor, press **Ctrl+A**, release it,
then press **A**. To exit `screen` and release the serial port, press
**Ctrl+A**, then **K**, then **Y**. Ctrl+A followed by D only detaches and
leaves the port in use.

Set
`CONFIG_POCKETGEN_AUTOSTART_CUBE=n` in the
[board configuration](boards/esp32p4_function_ev_board_esp32p4_hpcore.conf)
to start with the maze loaded and run it manually with Ctrl+R.

ESP32-P4 revision v0.1 has been tested but is experimental: the board
definition targets v1.3, and revision and ROM SHA-256 warnings appear at boot.

## License

pocketgen is [MIT licensed](LICENSE).

Third-party components retain their own licenses:

- [Microsoft Edit-derived editor](components/pocketgen-editor/THIRD_PARTY_NOTICES.md)
- [embedded-graphics-terminal-derived terminal](components/pocketgen-terminal/THIRD_PARTY_NOTICES.md)
- [MicroPython](components/micropython/LICENSE)
