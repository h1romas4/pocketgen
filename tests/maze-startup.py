import os
import re
import select
import subprocess
import sys
import termios
import time
import tty


env = dict(os.environ, SDL_VIDEODRIVER="dummy")
process = subprocess.Popen(
    [sys.argv[1]], stdout=subprocess.PIPE, stderr=subprocess.STDOUT, env=env
)
output = bytearray()
serial_fd = None


def wait_for(predicate, timeout=15):
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        if predicate(output):
            return
        ready, _, _ = select.select([process.stdout], [], [], 0.1)
        if ready:
            data = os.read(process.stdout.fileno(), 4096)
            if not data:
                raise RuntimeError("simulator exited unexpectedly")
            output.extend(data)
    raise RuntimeError("timed out waiting for simulator")


def observe_for(seconds):
    deadline = time.monotonic() + seconds
    while time.monotonic() < deadline:
        ready, _, _ = select.select([process.stdout], [], [], 0.1)
        if ready:
            data = os.read(process.stdout.fileno(), 4096)
            if not data:
                raise RuntimeError("simulator exited unexpectedly")
            output.extend(data)


try:
    wait_for(lambda data: re.search(rb"uart connected to pseudotty: (/dev/pts/\d+)", data))
    device = re.search(rb"uart connected to pseudotty: (/dev/pts/\d+)", output).group(1).decode()
    serial_fd = os.open(device, os.O_RDWR | os.O_NOCTTY)
    tty.setraw(serial_fd, termios.TCSANOW)
    wait_for(lambda data: b"Editor input ready on virtual UART" in data)
    observe_for(1)
    if b"MicroPython execution" in output:
        raise RuntimeError("sample ran before the user requested execution")
    start = len(output)
    os.write(serial_fd, b"\x12")
    wait_for(lambda data: b"MicroPython execution started" in data[start:])
    observe_for(1)
    if b"MicroPython script raised an exception" in output[start:]:
        raise RuntimeError("maze failed during execution")
    os.write(serial_fd, b"\x03")
    wait_for(lambda data: b"MicroPython execution interrupted" in data[start:])
    start = len(output)
    os.write(serial_fd, b"\x1b[21~\x1b[A\r\x7fprint('recovered')\r\x12")
    wait_for(lambda data: b"MicroPython execution completed" in data[start:])
    if b"<err>" in output:
        raise RuntimeError("simulator reported an error")
    print("Idle startup, user-run maze, Ctrl+C, and editor recovery passed")
except Exception:
    sys.stderr.write(output.decode(errors="replace"))
    raise
finally:
    if serial_fd is not None:
        os.close(serial_fd)
    process.terminate()
    try:
        process.wait(timeout=5)
    except subprocess.TimeoutExpired:
        process.kill()
        process.wait()
