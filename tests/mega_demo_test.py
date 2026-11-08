import pathlib
import sys
import types
import unittest
from unittest.mock import patch


SOURCE = pathlib.Path(__file__).resolve().parents[1] / "examples" / "mega_demo.py"
ENTRY = "run_demo()\n"


class Display:
    def __init__(self, width=510, height=568, inputs=()):
        self.width, self.height = width, height
        self.inputs = list(inputs)
        self.keys = []
        self.time = 0
        self.clears = 0
        self.lines = []
        self.tiles = []
        self.frames = []
        self.shown = False

    def graphics_size(self):
        return self.width, self.height

    def graphics_clear(self):
        self.clears += 1

    def graphics_line(self, x0, y0, x1, y1, color):
        assert 0 <= x0 < self.width and 0 <= x1 < self.width, (x0, x1)
        assert 0 <= y0 < self.height and 0 <= y1 < self.height, (y0, y1)
        self.lines.append((x0, y0, x1, y1, color))

    def graphics_pixel(self, x, y, color):
        self.graphics_line(x, y, x, y, color)

    def graphics_tile(self, x, y, rows, palette, scale=1):
        assert 0 <= x and x + len(rows[0]) * scale <= self.width
        assert 0 <= y and y + len(rows) * scale <= self.height
        self.tiles.append((x, y, rows, palette, scale))

    def graphics_show(self):
        self.shown = True

    def cls(self):
        pass

    def getkey(self):
        return self.keys.pop(0) if self.keys else None

    def ticks_ms(self):
        return self.time

    def ticks_diff(self, end, start):
        return end - start

    def sleep_ms(self, ms):
        self.time += ms
        self.frames.append((self.clears, len(self.lines)))
        if self.inputs:
            self.keys.extend(self.inputs.pop(0))


def load_demo(display):
    module = types.ModuleType("pocketgen")
    for name in ("cls", "getkey", "graphics_clear", "graphics_line", "graphics_pixel",
                 "graphics_show", "graphics_size", "graphics_tile", "sleep_ms",
                 "ticks_diff", "ticks_ms"):
        setattr(module, name, getattr(display, name))
    source = SOURCE.read_text()
    if not source.endswith(ENTRY):
        raise AssertionError("demo entry point changed")
    namespace = {"print": lambda *args: None}
    with patch.dict(sys.modules, {"pocketgen": module}):
        exec(compile(source[:-len(ENTRY)], str(SOURCE), "exec"), namespace)
    return namespace


class MegaDemoTests(unittest.TestCase):
    def test_all_scenes_and_frame_bounds(self):
        for width, height in ((240, 220), (510, 568), (800, 480), (240, 900)):
            with self.subTest(width=width, height=height):
                display = Display(width, height)
                demo = load_demo(display)
                demo["run_demo"](cycles=1, frames_per_scene=36, delay=1)
                self.assertEqual(display.clears, 5)
                self.assertEqual(len(display.frames), 180)
                self.assertEqual({scene for scene, _ in display.frames}, {1, 2, 3, 4, 5})
                self.assertTrue(display.shown)
                self.assertGreater(len({line[4] for line in display.lines}), 6)

    def test_time_and_keyboard_scene_changes(self):
        display = Display()
        load_demo(display)["run_demo"](scene_ms=2, cycles=1, delay=1)
        self.assertEqual([scene for scene, _ in display.frames], [1, 1, 2, 2, 3, 3, 4, 4, 5, 5])
        display = Display(inputs=[(" ",), ("q",)])
        load_demo(display)["run_demo"](cycles=1, delay=1)
        self.assertEqual([scene for scene, _ in display.frames], [1, 2])
        self.assertFalse(display.shown)
        display = Display(inputs=[("ESC",)])
        load_demo(display)["run_demo"](cycles=1, delay=1)
        self.assertEqual(len(display.frames), 1)

    def test_scenes_animate_and_repeat(self):
        display = Display()
        demo = load_demo(display)
        for name, args in (
            ("neon_grid", (510, 350, 530)),
            ("cube", (510, 54, 530)),
            ("waves", (510, 54, 530)),
            ("fireworks", (510, 54, 530)),
        ):
            with self.subTest(scene=name):
                self.assertNotEqual(demo[name](0, *args), demo[name](1, *args))
        state = demo["init_warp"]()
        self.assertNotEqual(demo["warp"](0, 510, 54, 530, state),
                            demo["warp"](1, 510, 54, 530, state))
        demo["run_demo"](cycles=2, frames_per_scene=1, delay=1)
        self.assertEqual(display.clears, 10)
        self.assertEqual(len(display.frames), 10)

    def test_small_screen(self):
        for size in ((239, 220), (240, 219)):
            display = Display(*size)
            with self.assertRaises(ValueError):
                load_demo(display)["run_demo"](cycles=1)
            self.assertEqual(display.clears, 0)


if __name__ == "__main__":
    unittest.main()
