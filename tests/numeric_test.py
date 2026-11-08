import importlib.util
import contextlib
import io
import math
import pathlib
import sys
import types
import unittest
from unittest.mock import patch

from plot_test import Graphics, load_plot_module


ROOT = pathlib.Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location("pocketgen_numeric", ROOT / "src/python/pocketgen_numeric.py")
numeric = importlib.util.module_from_spec(spec)
spec.loader.exec_module(numeric)


def fail(_):
    raise RuntimeError("callback failed")


class NumericTests(unittest.TestCase):
    def test_rc_demo_graphics_and_results(self):
        for size in ((128, 48), (512, 568)):
            with self.subTest(size=size):
                graphics = Graphics(*size)
                plot_module = load_plot_module(graphics)
                pocketgen = types.ModuleType("pocketgen")
                for name in ("graphics_size", "graphics_clear", "graphics_pixel", "graphics_line", "refresh"):
                    setattr(pocketgen, name, getattr(graphics, name))
                pocketgen.plot = plot_module.plot
                pocketgen.plot_position = plot_module.plot_position
                pocketgen.solve = numeric.solve
                pocketgen.integrate = numeric.integrate
                pocketgen.derivative = numeric.derivative
                shown = []
                pocketgen.graphics_show = lambda: shown.append(True)
                output = io.StringIO()
                with patch.dict(sys.modules, {"pocketgen": pocketgen}), contextlib.redirect_stdout(output):
                    source = (ROOT / "examples/numeric_rc.py").read_text()
                    result = {}
                    exec(compile(source, "numeric_rc.py", "exec"), result)
                self.assertAlmostEqual(result["half_time"], math.log(2), delta=1e-8)
                self.assertAlmostEqual(result["area"], 1 - math.exp(-3), delta=1e-8)
                self.assertAlmostEqual(result["slope"], -math.exp(-1), delta=1e-10)
                self.assertIn("Half-voltage time:", output.getvalue())
                self.assertIn("Voltage integral 0-3 s:", output.getvalue())
                self.assertIn("Slope at 1 s:", output.getvalue())
                self.assertEqual(shown, [True])
                self.assertEqual((graphics.clears, graphics.refreshes), (1, 2))
                for color in (0x07ff, 0xffe0, 0xf81f, 0x2108, 0x07e0, 0xffff):
                    self.assertTrue(any(line[-1] == color for line in graphics.lines), hex(color))
                for t, value, color in ((result["half_time"], 0.5, 0x07e0),
                                        (1, math.exp(-1), 0xffff)):
                    x, y = plot_module.plot_position(t, value, result["bounds"])
                    self.assertIn((x - 3, y, x + 3, y, color), graphics.lines)
                    self.assertIn((x, y - 3, x, y + 3, color), graphics.lines)

    def test_solve_roots_and_tolerance(self):
        root = numeric.solve(lambda x: x * x - 2, 0, 2, tol=1e-10)
        self.assertLessEqual(abs(root - math.sqrt(2)), 5e-11)
        self.assertEqual(numeric.solve(lambda x: x - 1, 1, 3), 1)
        self.assertEqual(numeric.solve(lambda x: x - 3, 1, 3), 3)
        self.assertEqual(numeric.solve(lambda x: x, -1, 1), 0)
        self.assertEqual(numeric.solve(lambda x: x - 0.1, 0, 1, tol=0.25, max_iter=2), 0.125)

    def test_solve_errors_and_callback(self):
        for args, kwargs in (
            ((lambda x: x*x + 1, -1, 1), {}),
            ((lambda x: x, 1, 1), {}),
            ((lambda x: x, -1, 1), {"tol": 0}),
            ((lambda x: x, -1, 1), {"max_iter": 0}),
            ((lambda x: x - 0.1, 0, 1), {"tol": 1e-12, "max_iter": 1}),
            ((lambda x: float("nan"), -1, 1), {}),
            ((lambda x: x, -1e308, 1e308), {}),
            ((lambda x: -1 if x == 1e16 else 1, 1e16, 1e16 + 2), {"tol": 0.1}),
        ):
            with self.subTest(args=args, kwargs=kwargs), self.assertRaises(ValueError):
                numeric.solve(*args, **kwargs)
        with self.assertRaises(TypeError):
            numeric.solve(1, 0, 1)
        with self.assertRaisesRegex(RuntimeError, "callback failed"):
            numeric.solve(fail, 0, 1)

    def test_integrate_accuracy_and_reverse(self):
        integral = numeric.integrate
        self.assertAlmostEqual(integral(lambda x: x**3, 0, 2, steps=2), 4, places=14)
        self.assertAlmostEqual(integral(math.sin, 0, math.pi, steps=100), 2, delta=2e-8)
        self.assertAlmostEqual(integral(math.sin, math.pi, 0, steps=100), -2, delta=2e-8)
        self.assertEqual(integral(fail, 2, 2), 0)
        self.assertAlmostEqual(integral(lambda _: 1e308, 0, 0.3, steps=2) / 1e308, 0.3)
        seen = []
        integral(lambda x: seen.append(x) or x, 0, 1, steps=4)
        self.assertEqual(seen, [0, 0.25, 0.5, 0.75, 1])

    def test_integrate_errors(self):
        for args, kwargs in (
            ((lambda x: x, 0, 1), {"steps": 0}),
            ((lambda x: x, 0, 1), {"steps": 3}),
            ((lambda x: x, 0, 1), {"steps": 2.0}),
            ((lambda x: x, -1e308, 1e308), {}),
            ((lambda x: float("inf"), 0, 1), {}),
            ((lambda x: x, 1e16, 1e16 + 2), {"steps": 2}),
        ):
            with self.subTest(args=args, kwargs=kwargs), self.assertRaises(ValueError):
                numeric.integrate(*args, **kwargs)
        with self.assertRaises(TypeError):
            numeric.integrate(None, 0, 1)
        with self.assertRaisesRegex(RuntimeError, "callback failed"):
            numeric.integrate(fail, 0, 1)

    def test_derivative_accuracy_and_errors(self):
        self.assertAlmostEqual(numeric.derivative(math.sin, 0), 1, delta=2e-11)
        self.assertAlmostEqual(numeric.derivative(lambda x: x*x, 3, h=0.001), 6, delta=1e-10)
        self.assertAlmostEqual(numeric.derivative(lambda x: 1e308*x, 0, h=1) / 1e308, 1)
        for x, h in ((0, 0), (0, -1), (float("inf"), 1), (1e16, 1e-5), (1e308, 1e308)):
            with self.subTest(x=x, h=h), self.assertRaises(ValueError):
                numeric.derivative(lambda t: t, x, h=h)
        with self.assertRaises(ValueError):
            numeric.derivative(lambda x: float("nan"), 0)
        with self.assertRaises(TypeError):
            numeric.derivative(42, 0)
        with self.assertRaisesRegex(RuntimeError, "callback failed"):
            numeric.derivative(fail, 0)


if __name__ == "__main__":
    unittest.main()
