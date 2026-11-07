import pathlib
import sys
import types
import unittest


SOURCE = pathlib.Path(__file__).resolve().parents[1] / "examples" / "starfield.py"
ENTRY = "run_demo()\n"


class Terminal:
    def __init__(self, cols, rows):
        self.cols, self.rows = cols, rows
        self.screen = [[" "] * cols for _ in range(rows)]
        self.colors = [[7] * cols for _ in range(rows)]
        self.x = self.y = 0
        self.fg = 7
        self.frames = []

    def size(self):
        return self.cols, self.rows

    def cls(self):
        self.screen = [[" "] * self.cols for _ in range(self.rows)]
        self.x = self.y = 0

    def locate(self, x, y):
        if not (0 <= x < self.cols and 0 <= y < self.rows):
            raise AssertionError(f"out-of-range position {(x, y)}")
        self.x, self.y = x, y

    def color(self, fg, bg=0):
        if not (0 <= fg <= 15 and 0 <= bg <= 15):
            raise AssertionError("invalid color")
        self.fg = fg

    def print(self, *values, end="\n"):
        for char in " ".join(str(value) for value in values) + end:
            if self.x >= self.cols or self.y >= self.rows or char == "\n":
                raise AssertionError("demo would wrap or scroll the terminal")
            self.screen[self.y][self.x] = char
            self.colors[self.y][self.x] = self.fg
            self.x += 1

    def sleep_ms(self, ms):
        self.frames.append((ms, tuple("".join(row) for row in self.screen)))


def load_demo(terminal, source_path=SOURCE):
    module = types.ModuleType("pocketgen")
    for name in ("cls", "color", "locate", "size", "sleep_ms"):
        setattr(module, name, getattr(terminal, name))
    source = source_path.read_text()
    if not source.endswith(ENTRY):
        raise AssertionError("demo entry point changed")
    previous = sys.modules.get("pocketgen")
    sys.modules["pocketgen"] = module
    try:
        namespace = {"print": terminal.print}
        exec(compile(source[:-len(ENTRY)], str(source_path), "exec"), namespace)
    finally:
        if previous is None:
            del sys.modules["pocketgen"]
        else:
            sys.modules["pocketgen"] = previous
    return namespace


class StarfieldDemoTests(unittest.TestCase):
    def test_animation_changes_and_stays_within_screen(self):
        for cols, rows in ((12, 6), (20, 10), (63, 34), (80, 24)):
            with self.subTest(cols=cols, rows=rows):
                terminal = Terminal(cols, rows)
                load_demo(terminal)["run_demo"](70, 1)
                self.assertEqual(len(terminal.frames), 70)
                self.assertTrue(all(delay == 1 for delay, _ in terminal.frames))
                frames = [frame for _, frame in terminal.frames]
                self.assertGreater(len(set(frames)), 10)
                self.assertTrue(any(glyph in row for frame in frames
                                    for row in frame[1:-1] for glyph in ".+*"))
                self.assertTrue(all(frame[-1] == " " * cols for frame in frames))

    def test_small_screen_exits_without_out_of_bounds_positions(self):
        for cols, rows in ((1, 1), (11, 6), (12, 5)):
            with self.subTest(cols=cols, rows=rows):
                terminal = Terminal(cols, rows)
                load_demo(terminal)["run_demo"](1, 1)
                self.assertFalse(terminal.frames)


if __name__ == "__main__":
    unittest.main()
