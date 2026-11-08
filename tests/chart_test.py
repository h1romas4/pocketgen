import importlib.util
import math
import pathlib
import sys
import types
import unittest
from array import array
from unittest.mock import patch

from plot_test import Graphics


ROOT = pathlib.Path(__file__).resolve().parents[1]


def load_charts(graphics):
    pocketgen = types.ModuleType("pocketgen")
    for name in ("graphics_size", "graphics_clear", "graphics_pixel", "graphics_line", "refresh"):
        setattr(pocketgen, name, getattr(graphics, name))
    with patch.dict(sys.modules, {"pocketgen": pocketgen}):
        plot_spec = importlib.util.spec_from_file_location("pocketgen_plot", ROOT / "src/python/pocketgen_plot.py")
        plot = importlib.util.module_from_spec(plot_spec)
        plot_spec.loader.exec_module(plot)
        with patch.dict(sys.modules, {"pocketgen_plot": plot}):
            spec = importlib.util.spec_from_file_location("pocketgen_charts", ROOT / "src/python/pocketgen_charts.py")
            charts = importlib.util.module_from_spec(spec)
            spec.loader.exec_module(charts)
    return charts


class ChartTests(unittest.TestCase):
    def setUp(self):
        self.graphics = Graphics(300, 180)
        self.charts = load_charts(self.graphics)

    def test_linear_fit(self):
        fit = self.charts.linear_fit
        slope, intercept, r2 = fit(array("d", [1, 2, 3, 4]), (3, 5, 7, 9))
        self.assertAlmostEqual(slope, 2)
        self.assertAlmostEqual(intercept, 1)
        self.assertAlmostEqual(r2, 1)
        self.assertEqual(fit((1, 2, 3), (4, 4, 4)), (0, 4, None))
        slope, intercept, r2 = fit([1e9, 1e9 + 1, 1e9 + 2], [1, 3, 5])
        self.assertAlmostEqual(slope, 2)
        self.assertAlmostEqual(intercept, -1999999999)
        self.assertAlmostEqual(r2, 1)
        for x, y in (([], []), ([1], [1]), ([1, 1], [2, 3]), ([1, 2], [1]),
                     ([1, float("nan")], [1, 2]), ([-1e308, 1e308], [1, 2])):
            with self.subTest(x=x, y=y), self.assertRaises(ValueError):
                fit(x, y)

    def test_scatter_fit_autorange_and_samples(self):
        calls = []

        def line(x):
            calls.append(x)
            return 2 * x + 1

        bounds = self.charts.scatter([1, 2, 3], (3, 5, 7), fit=line, grid=False)
        self.assertEqual(bounds, (1, 3, 3, 7))
        self.assertEqual(len(calls), 223)
        self.assertEqual((calls[0], calls[-1]), (1, 3))
        self.assertTrue(any(line[-1] == 0xFFE0 for line in self.graphics.lines))
        self.assertTrue(any(pixel[-1] == 0x07FF for pixel in self.graphics.pixels))
        self.assertEqual((self.graphics.clears, self.graphics.refreshes), (1, 1))

    def test_scatter_validation_gaps_and_clipping(self):
        scatter = self.charts.scatter
        for args, kwargs, exception in (
            (([], []), {}, ValueError),
            (([1], [1, 2]), {}, ValueError),
            (([1], [float("inf")]), {}, ValueError),
            (([1], [2]), {"fit": 42}, TypeError),
            (([1], [2]), {"ymin": 0}, ValueError),
            (([1], [2]), {"color": -1}, ValueError),
        ):
            with self.subTest(args=args, kwargs=kwargs), self.assertRaises(exception):
                scatter(*args, **kwargs)
        self.assertEqual(self.graphics.clears, 0)
        scatter([-2, 0, 2], [-2, 0, 2], fit=lambda x: 1 / x,
                xmin=-1, xmax=1, ymin=-1, ymax=1, color=0xF800, grid=False)
        points = [p for p in self.graphics.pixels if p[-1] == 0xF800]
        self.assertEqual(len(points), 5)
        self.assertTrue(all(76 <= x <= 298 and 11 <= y <= 166 for x, y, _ in points))
        with self.assertRaisesRegex(RuntimeError, "fit failed"):
            scatter([0, 1], [0, 1], fit=lambda x: (_ for _ in ()).throw(RuntimeError("fit failed")))

    def test_histogram_edges_and_out_of_range(self):
        counts, edges = self.charts.hist([1, 2, 2, 3], bins=2)
        self.assertEqual(counts, [1, 3])
        self.assertEqual(edges, [1, 2, 3])
        self.assertTrue(any(line[-1] == 0x07FF for line in self.graphics.lines))
        counts, edges = self.charts.hist(array("d", [7, 7, 7]), bins=4)
        self.assertEqual(counts, [0, 0, 3, 0])
        self.assertEqual((edges[0], edges[-1]), (6, 8))
        before = self.graphics.clears
        for data, kwargs in (([], {}), ([1, float("nan")], {}), ([1], {"bins": 0}),
                             ([1], {"bins": 224}), ([1, 2], {"xmin": 1.5, "xmax": 3}),
                             ([1], {"xmin": 0})):
            with self.subTest(data=data, kwargs=kwargs), self.assertRaises(ValueError):
                self.charts.hist(data, **kwargs)
        self.assertEqual(self.graphics.clears, before)

    def test_histogram_rounding_at_edges(self):
        counts, edges = self.charts.hist([0.0, 0.1, 0.2, 0.3], bins=3, xmin=0, xmax=0.3)
        self.assertEqual(sum(counts), 4)
        for value in (0.0, 0.1, 0.2, 0.3):
            expected = 2 if value == edges[-1] else next(
                i for i in range(3) if edges[i] <= value < edges[i + 1]
            )
            singleton = [0, 0, 0]
            singleton[expected] = 1
            actual, _ = self.charts.hist([value], bins=3, xmin=0, xmax=0.3)
            self.assertEqual(actual, singleton)

    def test_bar_positive_negative_and_labels(self):
        bounds = self.charts.bar([4, -2, 0], labels=["Mon", "TUE", "WED"])
        self.assertEqual(bounds, (-2, 4))
        bars = [line for line in self.graphics.lines if line[-1] == 0x07FF]
        self.assertTrue(any(y0 < y1 for _, y0, _, y1, _ in bars))
        self.assertTrue(any(y0 > y1 for _, y0, _, y1, _ in bars))
        self.assertTrue(any(pixel[-1] == 0xFFFF for pixel in self.graphics.pixels))
        before = self.graphics.clears
        for values, kwargs in (([], {}), ([1], {"labels": ["A", "B"]}),
                               ([1], {"labels": ["日本"]}), ([1], {"labels": ["A" * 50]}),
                               ([1], {"ymin": 1, "ymax": 3}), ([float("inf")], {})):
            with self.subTest(values=values, kwargs=kwargs), self.assertRaises(ValueError):
                self.charts.bar(values, **kwargs)
        self.assertEqual(self.graphics.clears, before)

    def test_pie_quadrants_and_validation(self):
        fractions = self.charts.pie([1, 1, 1, 1], labels=["A", "B", "C", "D"])
        self.assertEqual(fractions, [0.25] * 4)
        rows = [line for line in self.graphics.lines if line[0] <= 177 and line[2] <= 177]
        def at(x, y):
            return next(c for x0, row, x1, _, c in rows if row == y and x0 <= x <= x1)
        self.assertEqual(at(119, 60), 0x07FF)
        self.assertEqual(at(59, 60), 0xFFE0)
        self.assertEqual(at(59, 120), 0xF81F)
        self.assertEqual(at(119, 120), 0x07E0)
        for y in range(2, 179):
            dy = 90 - y
            span = int(math.sqrt(88 * 88 - dy * dy))
            for x in range(89 - span, 90 + span):
                at(x, y)
        before = self.graphics.clears
        for values, kwargs in (([], {}), ([0, 0], {}), ([-1, 2], {}),
                               ([float("nan")], {}), ([1], {"labels": ["日本"]}),
                               ([1, 2], {"colors": [1]}), ([1], {"colors": [65536]})):
            with self.subTest(values=values, kwargs=kwargs), self.assertRaises(ValueError):
                self.charts.pie(values, **kwargs)
        self.assertEqual(self.graphics.clears, before)

    def test_pie_large_numbers_and_zero_sector(self):
        fractions = self.charts.pie([0, 1e308, 1e308])
        self.assertEqual(fractions, [0, 0.5, 0.5])
        self.assertFalse(any(line[-1] == 0x07FF and line[0] < 178
                             for line in self.graphics.lines))

    def test_inputs_and_results_are_independent(self):
        x = [0, 1, 2]
        y = [2, 4, 6]
        labels = ["one", "two", "three"]
        colors = [0xF800, 0x07E0, 0x001F]
        self.charts.linear_fit(x, y)
        self.charts.scatter(x, y)
        counts, edges = self.charts.hist(y, bins=2)
        self.charts.bar(y, labels=labels)
        fractions = self.charts.pie(y, labels=labels, colors=colors)
        self.charts.hist([1, 1], bins=2)
        self.charts.pie([1, 1])
        self.assertEqual((x, y), ([0, 1, 2], [2, 4, 6]))
        self.assertEqual(labels, ["one", "two", "three"])
        self.assertEqual(colors, [0xF800, 0x07E0, 0x001F])
        self.assertEqual((counts, edges), ([1, 2], [2, 4, 6]))
        self.assertEqual(fractions, [1 / 6, 1 / 3, 1 / 2])


if __name__ == "__main__":
    unittest.main()
