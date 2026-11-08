import pathlib
import sys
import types
import unittest
from unittest.mock import patch


SOURCE = pathlib.Path(__file__).resolve().parents[1] / "examples" / "snake.py"
ENTRY = "run_demo()\n"


class GameGraphics:
    def __init__(self, width, height, inputs=()):
        self.width, self.height = width, height
        self.inputs = list(inputs)
        self.keys = []
        self.tiles = {}
        self.frames = []
        self.messages = []
        self.shown = False

    def graphics_size(self):
        return self.width, self.height

    def graphics_clear(self, color):
        self.tiles.clear()

    def graphics_line(self, x0, y0, x1, y1, color):
        assert 0 <= x0 < self.width and 0 <= x1 < self.width
        assert 0 <= y0 < self.height and 0 <= y1 < self.height

    def graphics_tile(self, x, y, rows, palette, scale=1):
        assert 0 <= x and x + len(rows[0]) * scale <= self.width
        assert 0 <= y and y + len(rows) * scale <= self.height
        self.tiles[(x, y)] = rows

    def graphics_show(self):
        self.shown = True

    def cls(self):
        pass

    def getkey(self):
        return self.keys.pop(0) if self.keys else None

    def sleep_ms(self, ms):
        self.frames.append(self.tiles.copy())
        if self.inputs:
            self.keys.extend(self.inputs.pop(0))

    def print(self, *values):
        self.messages.append(" ".join(str(value) for value in values))


def load_demo(graphics):
    module = types.ModuleType("pocketgen")
    for name in ("cls", "getkey", "graphics_clear", "graphics_line", "graphics_show",
                 "graphics_size", "graphics_tile", "sleep_ms"):
        setattr(module, name, getattr(graphics, name))
    source = SOURCE.read_text()
    if not source.endswith(ENTRY):
        raise AssertionError("demo entry point changed")
    namespace = {"print": graphics.print}
    with patch.dict(sys.modules, {"pocketgen": module}):
        exec(compile(source[:-len(ENTRY)], str(SOURCE), "exec"), namespace)
    return namespace


class SnakeTests(unittest.TestCase):
    def test_controls_reverse_and_single_turn_per_frame(self):
        graphics = GameGraphics(420, 450, [("LEFT",), ("UP",), ("RIGHT", "DOWN")])
        demo = load_demo(graphics)
        demo["run_demo"](4, 1)
        self.assertEqual(len(graphics.frames), 4)
        for frame, (x, y) in zip(graphics.frames, ((14, 11), (15, 11), (15, 10), (16, 10))):
            self.assertEqual(frame[(2 + x * 16, 28 + y * 16)], demo["HEAD"])
            self.assertEqual(list(frame.values()).count(demo["HEAD"]), 1)

    def test_food_growth_quit_and_game_over(self):
        graphics = GameGraphics(420, 450, [("q",)])
        demo = load_demo(graphics)
        positions = iter([(14, 11), (0, 0)])
        demo["new_food"] = lambda snake, width, height, state: (next(positions), state)
        demo["run_demo"](10, 1)
        self.assertEqual(len(graphics.frames), 1)
        frame = graphics.frames[0]
        self.assertEqual(list(frame.values()).count(demo["BODY"]), 3)
        self.assertEqual(frame[(2, 28)], demo["FOOD"])

        graphics = GameGraphics(196, 182)
        load_demo(graphics)["run_demo"](100, 1)
        self.assertIn("Game over. Score: 0", graphics.messages)
        self.assertTrue(graphics.shown)

    def test_food_avoids_snake_and_full_board(self):
        demo = load_demo(GameGraphics(420, 450))
        snake = [(0, 0), (1, 0), (0, 1)]
        food, _ = demo["new_food"](snake, 2, 2, 1)
        self.assertEqual(food, (1, 1))
        self.assertEqual(demo["new_food"](snake + [(1, 1)], 2, 2, 1), (None, 1))

    def test_win_and_escape(self):
        graphics = GameGraphics(420, 450)
        demo = load_demo(graphics)
        positions = iter([(14, 11), None])
        demo["new_food"] = lambda snake, width, height, state: (next(positions), state)
        demo["run_demo"](10, 1)
        self.assertIn("You win! Score: 1", graphics.messages)
        self.assertTrue(graphics.shown)

        graphics = GameGraphics(420, 450, [("ESC",)])
        load_demo(graphics)["run_demo"](10, 1)
        self.assertEqual(len(graphics.frames), 1)

    def test_supported_screen_sizes(self):
        for width, height in ((196, 182), (320, 240), (510, 568), (800, 480)):
            with self.subTest(width=width, height=height):
                graphics = GameGraphics(width, height)
                load_demo(graphics)["run_demo"](2, 1)
                self.assertEqual(len(graphics.frames), 2)
        with self.assertRaises(ValueError):
            load_demo(GameGraphics(195, 182))["run_demo"](1, 1)


if __name__ == "__main__":
    unittest.main()
