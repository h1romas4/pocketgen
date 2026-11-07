import pathlib
import unittest

from starfield_demo_test import Terminal, load_demo


SOURCE = pathlib.Path(__file__).resolve().parents[1] / "examples" / "cube.py"


class CubeDemoTests(unittest.TestCase):
    def test_projection_and_animation_fit_the_screen(self):
        for cols, rows in ((24, 12), (40, 20), (63, 34), (80, 24)):
            with self.subTest(cols=cols, rows=rows):
                terminal = Terminal(cols, rows)
                demo = load_demo(terminal, SOURCE)
                width = min(cols - 1, 49)
                height = min(rows - 2, 27)
                scale = min((width - 1) * 128 // 1200, (height - 1) * 128 // 600, 10)
                for frame in range(64):
                    points = demo["project"](frame, width, height, scale)
                    self.assertEqual(len(points), 8)
                    self.assertTrue(all(0 <= x < width and 0 <= y < height
                                        for x, y in points))
                self.assertEqual(len(demo["EDGES"]), 12)
                demo["run_demo"](64, 1)
                self.assertEqual(len(terminal.frames), 64)
                frames = [frame for _, frame in terminal.frames]
                self.assertGreater(len(set(frames)), 25)
                self.assertTrue(all(any("+" in row for row in frame[1:-1])
                                    for frame in frames))
                self.assertTrue(all(frame[-1] == " " * cols for frame in frames))

    def test_small_screen_exits_without_wrapping(self):
        for cols, rows in ((1, 1), (23, 12), (24, 11)):
            with self.subTest(cols=cols, rows=rows):
                terminal = Terminal(cols, rows)
                load_demo(terminal, SOURCE)["run_demo"](1, 1)
                self.assertFalse(terminal.frames)


if __name__ == "__main__":
    unittest.main()
