import os
import pty
import re
import select
import shutil
import signal
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
screen_pid = None
screen_name = f"pocketgen-test-{os.getpid()}"
via_screen = len(sys.argv) > 2 and sys.argv[2] == "--screen"
deadline = time.monotonic() + 15


def wait_for(message, start, timeout=10):
    end = time.monotonic() + timeout
    while time.monotonic() < end:
        if message in output[start:]:
            return
        if b"MicroPython script raised an exception" in output[start:]:
            raise RuntimeError("MicroPython raised an exception")
        ready, _, _ = select.select([process.stdout], [], [], 0.1)
        if ready:
            data = os.read(process.stdout.fileno(), 4096)
            if not data:
                raise RuntimeError(f"simulator exited while waiting for {message!r}")
            output.extend(data)
    raise RuntimeError(f"timed out waiting for {message!r}")


def replace_and_run(source):
    os.write(serial_fd, b"\x1b[21~\x1b[C\x1b[A\r\x7f" + source + b"\x12")


try:
    while time.monotonic() < deadline:
        ready, _, _ = select.select([process.stdout], [], [], 0.1)
        if ready:
            data = os.read(process.stdout.fileno(), 4096)
            if not data:
                raise RuntimeError("simulator exited before completing the UART test")
            output.extend(data)
        if serial_fd is None:
            match = re.search(rb"uart connected to pseudotty: (/dev/pts/\d+)", output)
            if match:
                device = match.group(1).decode()
                if via_screen:
                    if not shutil.which("screen"):
                        raise RuntimeError("screen is not installed")
                    screen_pid, serial_fd = pty.fork()
                    if screen_pid == 0:
                        os.environ["TERM"] = "xterm-256color"
                        os.execvp("screen", ["screen", "-U", "-S", screen_name, device])
                    time.sleep(0.5)
                else:
                    serial_fd = os.open(device, os.O_RDWR | os.O_NOCTTY)
                    tty.setraw(serial_fd, termios.TCSANOW)
        if serial_fd is not None and b"Editor input ready on virtual UART" in output:
            os.write(
                serial_fd,
                b"\x1b[21~\x1b[C\x1b[A\r\x7f"
                b"?\x1a"
                b"!\x1b[21~\x1b[C\r"
                b"print('uart-XY')"
                b"\x1b[D\x1b[D"
                b"\x1b[1;2D\x1b[1;2D"
                b"\x1b[21~\x1b[C\x1b[B\x1b[B\r"
                b"\x18\x16"
                b"\x1b[1;2D\x1b[1;2D\x03"
                b"ok"
                b"\x1b[1;2D\x1b[1;2D\x16"
                b"\x1b[1;2D\x1b[1;2D"
                b"ok\x1b[C\x1b[C\r"
                b"\x1b[21~\x1b[D\r",
            )
            break
    else:
        raise RuntimeError("simulator UART did not become ready")

    while time.monotonic() < deadline:
        ready, _, _ = select.select([process.stdout], [], [], 0.1)
        if ready:
            data = os.read(process.stdout.fileno(), 4096)
            if not data:
                raise RuntimeError("simulator exited during the UART test")
            output.extend(data)
            if b"UART editor execution test passed" in output:
                mode = "screen" if via_screen else "direct PTY"
                print(f"UART undo, selection, clipboard, menu, and MicroPython execution passed ({mode})")
                break
            if b"UART editor execution test failed" in output:
                raise RuntimeError("UART editor execution test failed")
    else:
        raise RuntimeError("timed out waiting for UART editor execution")

    start = len(output)
    os.write(serial_fd, b"\x07")
    wait_for(b"Cannot switch output view: no graphics available", start)

    start = len(output)
    select_all = b"\x01a" if via_screen else b"\x01"
    os.write(
        serial_fd,
        select_all + b"Xprint('uart-ok')\r# tailZ"
        b"\x1b[1;5H\x1b[3~\x1b[1;5F\x7f\x12",
    )
    wait_for(b"UART editor execution test passed", start)
    mode = "screen" if via_screen else "direct PTY"
    print(f"Ctrl+A and Ctrl+Home/End passed ({mode})")

    start = len(output)
    os.write(
        serial_fd,
        select_all + b"Xprint('uart-ok')\r" + b"#\r" * 32 + b"X#\r"
        + b"#\r" * 35 + b"\x1b[1;5H\x1b[6~\x1b[3~\x1b[5~\x1b[3~\x12",
    )
    wait_for(b"UART editor execution test passed", start)
    print(f"Page Up/Down passed ({mode})")

    for delay in (0, 1.5 if via_screen else 0.3):
        start = len(output)
        os.write(
            serial_fd,
            select_all + b"print('uart-bad')\r"
            + b"\x1b[1;5H\x06bad\r\x1b",
        )
        if delay:
            time.sleep(delay)
        # A leading '[' is ambiguous with an escape sequence until the
        # standalone Escape has timed out.
        os.write(serial_fd, (b"[\x7f" if delay else b"") + b"ok\x12")
        wait_for(b"UART editor execution test passed", start)
        if b"Invalid input: UnknownEscape" in output[start:]:
            raise RuntimeError("Escape and following input were not decoded separately")
    print(f"Ctrl+F search and Escape with immediate/delayed input passed ({mode})")

    start = len(output)
    os.write(
        serial_fd,
        select_all + b"print('uart-bad')\r# bad\r# bad"
        + b"\x1b[1;2D" * 3 + b"\x06"
        + b"\x10\x0e\x10\x10\x1bok\x12",
    )
    wait_for(b"UART editor execution test passed", start)
    print(f"Selection-prefilled Ctrl+P/Ctrl+N search passed ({mode})")

    for name, selection in ((b"starfield", b"\x1b[B"),
                            (b"cube", b"\x1b[B\x1b[B"),
                            (b"mexican hat", b"\x1b[A\x1b[A\x1b[A"),
                            (b"maze", b"")):
        start = len(output)
        os.write(serial_fd, b"\x1b[21~" + selection + b"\r")
        wait_for(b"Loaded " + name + b" demo", start)
        if b"MicroPython execution started" in output[start:]:
            raise RuntimeError("loading a demo unexpectedly ran it")
        os.write(serial_fd, b"\x12")
        wait_for(b"MicroPython execution started", start)
        started = output.find(b"MicroPython execution started", start)
        frame = b"Live output frame presented"
        for _ in range(5):
            wait_for(frame, started)
            started = output.index(frame, started) + len(frame)
        if name == b"mexican hat":
            wait_for(b"MicroPython execution completed", start)
        else:
            os.write(serial_fd, b"\x03")
            wait_for(b"MicroPython execution interrupted", start)
    print(f"Demo menu loading, animation, and interruption passed ({mode})")

    start = len(output)
    os.write(serial_fd, b"\x1b[21~\x1b[A\x1b[A\r")
    wait_for(b"Loaded sine graph demo", start)
    os.write(serial_fd, b"\x12")
    wait_for(b"MicroPython execution completed", start)
    if b"MicroPython script raised an exception" in output[start:]:
        raise RuntimeError("sine graph demo raised an exception")
    os.write(serial_fd, b"\x07")
    wait_for(b"Output view: text", start)
    os.write(serial_fd, b"\x07")
    wait_for(b"Output view: graphics", start)
    print(f"Sine graph demo menu and execution passed ({mode})")

    start = len(output)
    os.write(serial_fd, b"\x1b[21~\x1b[A\r")
    wait_for(b"Loaded mexican hat graphics demo", start)
    os.write(serial_fd, b"\x12")
    wait_for(b"MicroPython execution started", start)
    wait_for(b"Live output frame presented", start)
    wait_for(b"MicroPython execution completed", start, timeout=30)
    os.write(serial_fd, b"\x07")
    wait_for(b"Output view: text", start)
    os.write(serial_fd, b"\x07")
    wait_for(b"Output view: graphics", start)
    print(f"Graphics Mexican hat demo menu, drawing, and view toggle passed ({mode})")

    start = len(output)
    replace_and_run(
        b"from pocketgen import graphics_clear, graphics_pixel, graphics_show, sleep_ms\r"
        b"graphics_clear()\r"
        b"print('retained-text')\r"
        b"graphics_show()\r"
        b"while True:\r"
        b" graphics_pixel(0, 0, 0xffff)\r"
        b" print('running')\r"
        b" graphics_show()\r"
        b" sleep_ms(40)\r"
    )
    wait_for(b"Live output frame presented", start)
    os.write(serial_fd, b"\x07")
    wait_for(b"Output view: text", start)
    os.write(serial_fd, b"\x07")
    wait_for(b"Output view: graphics", start)
    start = len(output)
    os.write(serial_fd, b"\x03")
    wait_for(b"Output view: text (Python exception)", start)
    wait_for(b"MicroPython execution interrupted", start)
    print(f"Output view shortcut during and after execution passed ({mode})")

    start = len(output)
    replace_and_run(
        b"from pocketgen import sleep_ms\r"
        b"print('live-frame')\r"
        b"sleep_ms(5000)\r"
        b"print('should-not-run')\r"
    )
    wait_for(b"MicroPython execution started", start)
    started = output.find(b"MicroPython execution started", start)
    wait_for(b"Live output frame presented", started)
    os.write(serial_fd, b"\x03")
    wait_for(b"MicroPython execution interrupted", start)

    start = len(output)
    replace_and_run(b"print('loop-frame')\rwhile True:\r pass\r")
    wait_for(b"MicroPython execution started", start)
    started = output.find(b"MicroPython execution started", start)
    wait_for(b"Live output frame presented", started)
    os.write(serial_fd, b"\x03")
    wait_for(b"MicroPython execution interrupted", start)

    start = len(output)
    replace_and_run(b"print('uart-ok')\r")
    wait_for(b"UART editor execution test passed", start)
    mode = "screen" if via_screen else "direct PTY"
    print(f"Live output, sleep and tight-loop interruption, and recovery passed ({mode})")
except Exception:
    sys.stderr.write(output.decode(errors="replace"))
    raise
finally:
    if screen_pid:
        subprocess.run(["screen", "-S", screen_name, "-X", "quit"], check=False,
                       stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        try:
            os.kill(screen_pid, signal.SIGTERM)
        except ProcessLookupError:
            pass
        os.waitpid(screen_pid, 0)
    if serial_fd is not None:
        os.close(serial_fd)
    process.terminate()
    try:
        process.wait(timeout=5)
    except subprocess.TimeoutExpired:
        process.kill()
        process.wait()
