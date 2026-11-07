import pathlib
import unittest

from starfield_demo_test import Terminal, load_demo


SOURCE = pathlib.Path(__file__).resolve().parents[1] / "examples" / "mexican_hat.py"


class MexicanHatDemoTests(unittest.TestCase):
    def test_surface_and_progressive_drawing(self):
        for cols, rows in ((24, 12), (40, 20), (63, 34), (80, 24)):
            with self.subTest(cols=cols, rows=rows):
                terminal = Terminal(cols, rows)
                demo = load_demo(terminal, SOURCE)
                self.assertEqual(demo["radius"](0, 0), 0)
                self.assertEqual(demo["radius"](3, 4), 80)
                width = cols - 1 if cols - 1 < 59 else 59
                height = rows - 2 if rows - 2 < 27 else 27
                self.assertLess(demo["project"](0, 0, width, height)[1],
                                demo["project"](12, 0, width, height)[1])
                demo["run_demo"](0, 1)
                self.assertEqual(len(terminal.frames), 13)
                frames = [frame for _, frame in terminal.frames]
                self.assertGreater(len(set(frames)), 5)
                self.assertTrue(any("*" in row for row in frames[-1]))
                self.assertEqual(frames[-1][-1], " " * cols)

    def test_small_screen_exits_without_wrapping(self):
        for cols, rows in ((1, 1), (23, 12), (24, 11)):
            with self.subTest(cols=cols, rows=rows):
                terminal = Terminal(cols, rows)
                load_demo(terminal, SOURCE)["run_demo"](1, 1)
                self.assertFalse(terminal.frames)


if __name__ == "__main__":
    unittest.main()
