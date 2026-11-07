import pathlib
import sys
import types
import unittest
from unittest.mock import patch


SOURCE = pathlib.Path(__file__).resolve().parents[1] / "examples" / "mexican_hat_graphics.py"
ENTRY = "run_demo()\n"


class Graphics:
    def __init__(self, width, height):
        self.width = width
        self.height = height
        self.pixels = [[0] * width for _ in range(height)]
        self.frames = []
        self.delays = []

    def graphics_size(self):
        return self.width, self.height

    def graphics_clear(self, color=0):
        self.pixels = [[color] * self.width for _ in range(self.height)]

    def graphics_pixel(self, x, y, color):
        if not (0 <= x < self.width and 0 <= y < self.height):
            raise AssertionError(f"out-of-range position {(x, y)}")
        if not 0 <= color <= 0xffff:
            raise AssertionError(f"invalid color {color}")
        self.pixels[y][x] = color

    def refresh(self):
        self.frames.append(sum(pixel != 0 for row in self.pixels for pixel in row))

    def sleep_ms(self, delay):
        if delay < 0:
            raise AssertionError("negative delay")
        self.delays.append(delay)


def load_demo(graphics):
    module = types.ModuleType("pocketgen")
    for name in ("graphics_size", "graphics_clear", "graphics_pixel", "refresh", "sleep_ms"):
        setattr(module, name, getattr(graphics, name))
    source = SOURCE.read_text()
    if not source.endswith(ENTRY):
        raise AssertionError("demo entry point changed")
    namespace = {}
    with patch.dict(sys.modules, {"pocketgen": module}):
        exec(compile(source[:-len(ENTRY)], str(SOURCE), "exec"), namespace)
    return namespace


class MexicanHatGraphicsDemoTests(unittest.TestCase):
    def test_surface_and_progressive_drawing(self):
        for width, height in ((24, 12), (80, 48), (320, 240), (512, 568)):
            with self.subTest(width=width, height=height):
                graphics = Graphics(width, height)
                demo = load_demo(graphics)
                self.assertEqual(demo["surface"](0, 0), 70.0)
                self.assertGreater(demo["surface"](44, 0), 90.0)
                self.assertLess(demo["surface"](136, 0), -90.0)
                z = demo["surface"](0, 0)
                self.assertEqual(demo["project"](0, 0, z, 160, 100), (80, 23))
                self.assertEqual(demo["project"](0, 60, z, 160, 100), (70, 13))
                demo["run_demo"](0, 0)
                self.assertEqual(len(graphics.frames), 21)
                self.assertEqual(graphics.frames, sorted(graphics.frames))
                self.assertGreater(graphics.frames[-1], width)
                self.assertGreaterEqual(len({p for row in graphics.pixels for p in row} - {0}), 4)

    def test_projection_rounds_only_at_output_resolution(self):
        demo = load_demo(Graphics(512, 568))
        for width, height in ((160, 100), (320, 200), (512, 320), (1024, 640)):
            for depth in range(-180, 181, 6):
                for x in range(-180, 181, 4):
                    z = demo["surface"](x, depth)
                    actual_x, actual_y = demo["project"](x, depth, z, width, height)
                    expected_x = (80 + x / 3 - depth / 6) * (width - 1) / 159
                    expected_y = (40 - depth / 6 - z / 4) * (height - 1) / 99
                    self.assertLessEqual(abs(actual_x - expected_x), 0.500000001)
                    self.assertLessEqual(abs(actual_y - expected_y), 0.500000001)

    def test_partial_and_small_displays(self):
        graphics = Graphics(80, 48)
        load_demo(graphics)["run_demo"](3, 0)
        self.assertEqual(len(graphics.frames), 1)
        for size in ((23, 12), (24, 11)):
            with self.subTest(size=size):
                graphics = Graphics(*size)
                with self.assertRaisesRegex(ValueError, "graphics area is too small"):
                    load_demo(graphics)["run_demo"]()
                self.assertFalse(graphics.frames)

    def test_default_delay_and_optional_wait(self):
        default = Graphics(80, 48)
        load_demo(default)["run_demo"]()
        immediate = Graphics(80, 48)
        load_demo(immediate)["run_demo"](delay=0)
        delayed = Graphics(80, 48)
        load_demo(delayed)["run_demo"](delay=7)
        self.assertEqual(default.delays, [])
        self.assertEqual(immediate.delays, [])
        self.assertEqual(delayed.delays, [7] * 21)
        self.assertEqual(default.frames, immediate.frames)
        self.assertEqual(default.frames, delayed.frames)
        self.assertEqual(default.pixels, immediate.pixels)
        self.assertEqual(default.pixels, delayed.pixels)

    def test_horizon_hides_lower_lines_and_clips_to_graph(self):
        graphics = Graphics(16, 12)
        demo = load_demo(graphics)
        horizon = [6] * 8
        demo["plot_line"](horizon, 6, 4, 3, (-2, 2), (9, 2), 0x001f)
        demo["plot_line"](horizon, 6, 4, 3, (0, 4), (7, 4), 0xf800)
        self.assertEqual(horizon, [2] * 8)
        self.assertEqual(graphics.pixels[5][4:12], [0x001f] * 8)
        self.assertEqual(graphics.pixels[7][4:12], [0] * 8)
        self.assertEqual(graphics.pixels[5][:4], [0] * 4)
        self.assertEqual(graphics.pixels[5][12:], [0] * 4)


if __name__ == "__main__":
    unittest.main()
