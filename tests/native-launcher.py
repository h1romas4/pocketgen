import os
import pty
import select
import signal
import subprocess
import sys
import termios
import time
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
LAUNCHER = ROOT / "scripts/native-sim.py"
SIMULATOR = Path(sys.argv[1]).resolve()


def wait_for(fd, output, message, start=0, timeout=20):
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        if message in output[start:]:
            return
        ready, _, _ = select.select([fd], [], [], 0.1)
        if ready:
            data = os.read(fd, 4096)
            if not data:
                raise RuntimeError(f"launcher exited while waiting for {message!r}")
            output.extend(data)
    raise RuntimeError(f"timed out waiting for {message!r}")


def exercise_launcher(interrupt):
    master, slave = pty.openpty()
    initial = termios.tcgetattr(slave)
    env = dict(os.environ, SDL_VIDEODRIVER="dummy")
    process = subprocess.Popen(
        [sys.executable, str(LAUNCHER), "--simulator", str(SIMULATOR)],
        stdin=slave,
        stdout=slave,
        stderr=slave,
        env=env,
    )
    output = bytearray()
    try:
        wait_for(master, output, b"UART connected. Ctrl+] exits")
        children = Path(f"/proc/{process.pid}/task/{process.pid}/children").read_text().split()
        if len(children) != 1:
            raise RuntimeError(f"expected one simulator child, found {children}")
        simulator_pid = int(children[0])
        if interrupt:
            process.send_signal(signal.SIGTERM)
            expected_status = 128 + signal.SIGTERM
        else:
            start = len(output)
            os.write(master, b"\x12")
            wait_for(master, output, b"MicroPython execution started", start)
            os.write(master, b"\x03")
            wait_for(master, output, b"MicroPython execution interrupted", start)
            start = len(output)
            os.write(master, b"\x1b[21~\x1b[C\x1b[A\r\x7fprint('recovered')\r\x12")
            wait_for(master, output, b"MicroPython execution completed", start)
            os.write(master, b"\x1d")
            expected_status = 0
        status = process.wait(timeout=10)
        if status != expected_status:
            raise RuntimeError(f"launcher exited with {status}, expected {expected_status}")
        if termios.tcgetattr(slave) != initial:
            raise RuntimeError("launcher did not restore terminal settings")
        if Path(f"/proc/{simulator_pid}").exists():
            raise RuntimeError("launcher left its simulator process running")
    except Exception:
        sys.stderr.write(output.decode(errors="replace"))
        raise
    finally:
        if process.poll() is None:
            process.kill()
            process.wait()
        os.close(master)
        os.close(slave)


exercise_launcher(False)
exercise_launcher(True)

master, slave = pty.openpty()
initial = termios.tcgetattr(slave)
process = subprocess.Popen(
    [sys.executable, str(LAUNCHER), "--simulator", "/bin/false"],
    stdin=slave,
    stdout=subprocess.PIPE,
    stderr=subprocess.PIPE,
)
try:
    _, stderr = process.communicate(timeout=10)
    if process.returncode == 0 or b"exited before the UART was ready" not in stderr:
        raise RuntimeError("launcher did not report simulator startup failure")
    if termios.tcgetattr(slave) != initial:
        raise RuntimeError("startup failure changed terminal settings")
finally:
    if process.poll() is None:
        process.kill()
        process.wait()
    os.close(master)
    os.close(slave)

result = subprocess.run(
    [sys.executable, str(LAUNCHER), "--simulator", str(SIMULATOR)],
    stdin=subprocess.DEVNULL,
    capture_output=True,
    text=True,
)
if result.returncode == 0 or "interactive terminal" not in result.stderr:
    raise RuntimeError("launcher did not report non-interactive input")
print("Launcher UART, Ctrl+C, exit, signals, startup failure, and terminal restoration passed")
