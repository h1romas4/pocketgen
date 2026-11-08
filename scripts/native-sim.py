#!/usr/bin/env python3
"""Launch native-sim and connect this terminal to its virtual UART."""

import argparse
import os
import re
import selectors
import signal
import subprocess
import sys
import termios
import time
import tty
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
DEFAULT_SIMULATOR = ROOT / "build/workspace/native_display/zephyr/zephyr.exe"
PTY_PATTERN = re.compile(rb"uart connected to pseudotty: (/dev/pts/\d+)")
READY = b"Editor input ready on virtual UART"
QUIT = b"\x1d"


def write_all(fd, data):
    while data:
        written = os.write(fd, data)
        if written == 0:
            raise OSError("write returned zero bytes")
        data = data[written:]


def terminate_process(process):
    if process.poll() is None:
        process.terminate()
        try:
            process.wait(timeout=5)
        except subprocess.TimeoutExpired:
            process.kill()
            process.wait()


def run(simulator):
    if not os.isatty(sys.stdin.fileno()):
        raise RuntimeError("standard input must be an interactive terminal")
    if not simulator.is_file() or not os.access(simulator, os.X_OK):
        raise RuntimeError(
            f"simulator executable not found: {simulator}\n"
            "Build it first: west build -b native_sim/native/64 . -d build/workspace/native_display"
        )

    process = subprocess.Popen(
        [str(simulator)],
        stdin=subprocess.DEVNULL,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
    )
    saved_terminal = None
    serial_fd = None
    deadline = time.monotonic() + 30
    recent_output = b""
    connected = False
    try:
        with selectors.DefaultSelector() as selector:
            selector.register(process.stdout, selectors.EVENT_READ, "log")
            while True:
                events = selector.select(timeout=0.2)
                if not connected and time.monotonic() >= deadline:
                    raise RuntimeError("timed out waiting for the simulator UART")
                for key, _ in events:
                    if key.data == "log":
                        data = os.read(process.stdout.fileno(), 4096)
                        if not data:
                            status = process.wait()
                            if not connected:
                                raise RuntimeError(
                                    f"simulator exited before the UART was ready (status {status})"
                                )
                            return 128 - status if status < 0 else status
                        write_all(sys.stdout.fileno(), data)
                        recent_output = (recent_output + data)[-4096:]
                        if serial_fd is None:
                            match = PTY_PATTERN.search(recent_output)
                            if match:
                                serial_fd = os.open(
                                    os.fsdecode(match.group(1)), os.O_RDWR | os.O_NOCTTY
                                )
                                tty.setraw(serial_fd, termios.TCSANOW)
                                selector.register(serial_fd, selectors.EVENT_READ, "uart")
                        if serial_fd is not None and not connected and READY in recent_output:
                            saved_terminal = termios.tcgetattr(sys.stdin.fileno())
                            tty.setraw(sys.stdin.fileno(), termios.TCSANOW)
                            raw_terminal = termios.tcgetattr(sys.stdin.fileno())
                            raw_terminal[1] = saved_terminal[1]
                            termios.tcsetattr(sys.stdin.fileno(), termios.TCSANOW, raw_terminal)
                            selector.register(sys.stdin, selectors.EVENT_READ, "input")
                            connected = True
                            write_all(
                                sys.stdout.fileno(),
                                b"\nUART connected. Ctrl+] exits; Ctrl+C is sent to pocketgen.\n",
                            )
                    elif key.data == "uart":
                        data = os.read(serial_fd, 4096)
                        if not data:
                            raise RuntimeError("simulator UART disconnected")
                        write_all(sys.stdout.fileno(), data)
                    else:
                        data = os.read(sys.stdin.fileno(), 4096)
                        if not data:
                            raise RuntimeError("terminal input closed")
                        before_quit, quit_key, _ = data.partition(QUIT)
                        if before_quit:
                            write_all(serial_fd, before_quit)
                        if quit_key:
                            return 0
    finally:
        try:
            if saved_terminal is not None:
                termios.tcsetattr(sys.stdin.fileno(), termios.TCSANOW, saved_terminal)
        finally:
            try:
                if serial_fd is not None:
                    os.close(serial_fd)
            finally:
                terminate_process(process)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--simulator", type=Path, default=DEFAULT_SIMULATOR)
    args = parser.parse_args()

    def stop_on_signal(signum, _frame):
        raise SystemExit(128 + signum)

    signal.signal(signal.SIGTERM, stop_on_signal)
    signal.signal(signal.SIGINT, stop_on_signal)
    try:
        return run(args.simulator.resolve())
    except (OSError, RuntimeError) as error:
        print(f"native-sim: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
