import importlib.util
import pathlib
import sys
import types
import unittest
from unittest.mock import patch


ROOT = pathlib.Path(__file__).resolve().parents[1]


class TileTests(unittest.TestCase):
    def setUp(self):
        self.pixels = [[99] * 8 for _ in range(8)]
        module = types.ModuleType("pocketgen")
        module.graphics_size = lambda: (8, 8)

        def line(x0, y0, x1, y1, color):
            self.assertEqual(y0, y1)
            for x in range(x0, x1 + 1):
                self.pixels[y0][x] = color

        module.graphics_line = line
        spec = importlib.util.spec_from_file_location("pocketgen_tiles", ROOT / "src/python/pocketgen_tiles.py")
        tiles = importlib.util.module_from_spec(spec)
        with patch.dict(sys.modules, {"pocketgen": module}):
            spec.loader.exec_module(tiles)
        self.draw = tiles.graphics_tile

    def test_scale_palette_transparency_and_edges(self):
        self.draw(2, 2, ("ab", ".a"), {"a": 1, "b": 2, ".": None}, scale=2)
        self.assertEqual(self.pixels[2][2:6], [1, 1, 2, 2])
        self.assertEqual(self.pixels[3][2:6], [1, 1, 2, 2])
        self.assertEqual(self.pixels[4][2:6], [99, 99, 1, 1])
        self.assertEqual(self.pixels[5][2:6], [99, 99, 1, 1])
        self.draw(7, 7, ("a",), {"a": 65535})
        self.assertEqual(self.pixels[7][7], 65535)

    def test_invalid_inputs_do_not_draw(self):
        cases = [
            ((0, 0, (), {}), ValueError),
            ((0, 0, ("",), {}), ValueError),
            ((0, 0, ("a", "aa"), {"a": 1}), ValueError),
            ((0, 0, ("ab",), {"a": 1}), ValueError),
            ((0, 0, ("a",), {"a": -1}), ValueError),
            ((0, 0, ("a",), {"a": 65536}), ValueError),
            ((0, 0, ("a",), {"a": "red"}), ValueError),
            ((-1, 0, ("a",), {"a": 1}), ValueError),
            ((7, 7, ("aa",), {"a": 1}), ValueError),
            ((0, 0, ("a",), {"a": 1}, 0), ValueError),
            ((0.5, 0, ("a",), {"a": 1}), TypeError),
            ((0, 0, ("a",), {"a": 1}, 1.5), TypeError),
            ((0, 0, ("a",), []), TypeError),
        ]
        for args, error in cases:
            with self.subTest(args=args), self.assertRaises(error):
                self.draw(*args)
            self.assertTrue(all(pixel == 99 for row in self.pixels for pixel in row))


if __name__ == "__main__":
    unittest.main()
