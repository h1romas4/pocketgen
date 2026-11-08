import importlib.util
import math
import pathlib
import sys
import types
import unittest
from unittest.mock import patch


ROOT = pathlib.Path(__file__).resolve().parents[1]


class Graphics:
    def __init__(self, width=200, height=100):
        self.width = width
        self.height = height
        self.lines = []
        self.pixels = []
        self.clears = 0
        self.refreshes = 0

    def graphics_size(self):
        return self.width, self.height

    def graphics_clear(self):
        self.clears += 1

    def graphics_pixel(self, x, y, color):
        assert 0 <= x < self.width and 0 <= y < self.height
        self.pixels.append((x, y, color))

    def graphics_line(self, x0, y0, x1, y1, color):
        assert 0 <= x0 < self.width and 0 <= x1 < self.width
        assert 0 <= y0 < self.height and 0 <= y1 < self.height
        self.lines.append((x0, y0, x1, y1, color))

    def refresh(self):
        self.refreshes += 1


def load_plot_module(graphics):
    module = types.ModuleType("pocketgen")
    for name in ("graphics_size", "graphics_clear", "graphics_pixel", "graphics_line", "refresh"):
        setattr(module, name, getattr(graphics, name))
    spec = importlib.util.spec_from_file_location("pocketgen_plot", ROOT / "src/python/pocketgen_plot.py")
    plot_module = importlib.util.module_from_spec(spec)
    with patch.dict(sys.modules, {"pocketgen": module}):
        spec.loader.exec_module(plot_module)
    return plot_module


def load_plot(graphics):
    return load_plot_module(graphics).plot


class PlotTests(unittest.TestCase):
    def test_plot_position(self):
        graphics = Graphics()
        module = load_plot_module(graphics)
        bounds = module.plot(lambda x: x, -1, 1)
        self.assertEqual(module.plot_position(-1, -1, bounds), (76, 86))
        self.assertEqual(module.plot_position(1, 1, bounds), (198, 11))
        self.assertEqual(module.plot_position(0, 0, bounds), (137, 49))
        for x, y, limits in ((2, 0, bounds), (0, -2, bounds),
                             (math.nan, 0, bounds), (0, math.inf, bounds),
                             (0, 0, (0, 0, -1, 1))):
            with self.subTest(x=x, y=y, limits=limits), self.assertRaises(ValueError):
                module.plot_position(x, y, limits)
        with self.assertRaises(ValueError):
            load_plot_module(Graphics(127, 100)).plot_position(0, 0, (-1, 1, -1, 1))

    def test_auto_range_multiple_colors_and_single_sampling(self):
        graphics = Graphics()
        calls = [[], []]

        def first(x):
            calls[0].append(x)
            return x

        def second(x):
            calls[1].append(x)
            return -2 * x

        bounds = load_plot(graphics)((first, second), -2, 2)
        self.assertEqual(bounds, (-2, 2, -4, 4))
        self.assertEqual(len(calls[0]), 123)
        self.assertEqual(calls[0], calls[1])
        self.assertEqual((calls[0][0], calls[0][-1]), (-2, 2))
        self.assertTrue(any(line[-1] == 0x07FF for line in graphics.lines))
        self.assertTrue(any(line[-1] == 0xFFE0 for line in graphics.lines))
        self.assertEqual((graphics.clears, graphics.refreshes), (1, 1))

    def test_constant_range_and_single_sample(self):
        graphics = Graphics()
        plot = load_plot(graphics)
        self.assertEqual(plot(lambda x: 5, colors=0xF800), (-10, 10, 4, 6))
        self.assertTrue(any(pixel[-1] == 0xF800 for pixel in graphics.pixels))
        graphics = Graphics()
        plot = load_plot(graphics)
        plot(lambda x: 0 if x == 0 else None, -1, 1, -1, 1, colors=0xF800, grid=False)
        self.assertEqual([p for p in graphics.pixels if p[-1] == 0xF800], [(137, 49, 0xF800)])
        self.assertFalse(any(line[-1] == 0xF800 for line in graphics.lines))

    def test_gaps_and_clipping(self):
        graphics = Graphics()
        plot = load_plot(graphics)

        def reciprocal(x):
            return 1 / x

        plot(reciprocal, -1, 1, -3, 3, colors=0xF800, grid=False)
        curves = [line for line in graphics.lines if line[-1] == 0xF800]
        self.assertTrue(curves)
        self.assertFalse(any(x0 == 136 or x1 == 138 for x0, _, x1, _, _ in curves))
        self.assertFalse(any(line[1] == 11 and line[3] == 86 for line in curves))

    def test_nonfinite_samples_and_errors(self):
        graphics = Graphics()
        plot = load_plot(graphics)
        plot(lambda x: math.nan if x < 0 else math.sqrt(x), -1, 1)
        for callback in (lambda x: None, lambda x: float("inf"),
                         lambda x: math.sqrt(-1)):
            with self.assertRaisesRegex(ValueError, "no finite samples"):
                plot(callback)
        before = graphics.clears
        with self.assertRaisesRegex(RuntimeError, "callback bug"):
            plot(lambda x: (_ for _ in ()).throw(RuntimeError("callback bug")))
        with self.assertRaises(TypeError):
            plot(lambda x: [])
        self.assertEqual(graphics.clears, before)

    def test_validation(self):
        graphics = Graphics()
        plot = load_plot(graphics)
        f = lambda x: x
        for args, kwargs, exception in (
            (([],), {}, TypeError),
            ((42,), {}, TypeError),
            ((f,), {"xmin": 1, "xmax": 1}, ValueError),
            ((f,), {"xmin": float("nan")}, ValueError),
            ((f,), {"xmin": -1e308, "xmax": 1e308}, ValueError),
            ((f,), {"ymin": 0}, ValueError),
            ((f,), {"ymin": 2, "ymax": 1}, ValueError),
            (((f, f),), {"colors": (1,)}, ValueError),
            ((f,), {"colors": -1}, ValueError),
            ((f,), {"colors": 65536}, ValueError),
        ):
            with self.subTest(args=args, kwargs=kwargs), self.assertRaises(exception):
                plot(*args, **kwargs)
        self.assertEqual(graphics.clears, 0)
        with self.assertRaisesRegex(ValueError, "too small"):
            load_plot(Graphics(127, 100))(f)

    def test_demos(self):
        for name in ("plot_trig.py", "plot_projectile.py", "plot_growth.py"):
            graphics = Graphics(300, 180)
            module = types.ModuleType("pocketgen")
            module.plot = load_plot(graphics)
            with self.subTest(name=name), patch.dict(sys.modules, {"pocketgen": module}):
                source = (ROOT / "examples" / name).read_text()
                exec(compile(source, name, "exec"), {})
                self.assertEqual((graphics.clears, graphics.refreshes), (1, 1))
                self.assertTrue(any(line[-1] == 0x07FF for line in graphics.lines))
                self.assertTrue(any(line[-1] == 0xFFE0 for line in graphics.lines))


if __name__ == "__main__":
    unittest.main()
