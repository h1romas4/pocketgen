import pathlib
import sys
import types
import unittest


SOURCE = pathlib.Path(__file__).resolve().parents[1] / "examples" / "terminal_demo.py"
ENTRY = "run_demo()\n"


class Terminal:
    def __init__(self, cols, rows):
        self.cols, self.rows = cols, rows
        self.x = self.y = 0
        self.screen = []
        self.backgrounds = []
        self.fg, self.bg = 7, 0
        self.mazes = []
        self.steps = []
        self.goals = 0
        self.waits = []

    def size(self):
        return self.cols, self.rows

    def cls(self):
        if self.screen:
            self.mazes.append(self.screen)
        self.screen = [[" "] * self.cols for _ in range(self.rows)]
        self.backgrounds = [[self.bg] * self.cols for _ in range(self.rows)]
        self.x = self.y = 0
        self.steps.append([])

    def locate(self, x, y):
        if not (0 <= x < self.cols and 0 <= y < self.rows):
            raise AssertionError(f"out-of-range position {(x, y)}")
        self.x, self.y = x, y

    def color(self, fg, bg=0):
        self.fg, self.bg = fg, bg

    def sleep_ms(self, ms):
        self.waits.append(ms)

    def print(self, *values, end="\n"):
        text = " ".join(str(value) for value in values) + end
        for char in text:
            if char == "\n":
                self.x = 0
                self.y += 1
            else:
                if self.x == self.cols:
                    self.x = 0
                    self.y += 1
                if self.y == self.rows:
                    self.screen.pop(0)
                    self.screen.append([" "] * self.cols)
                    self.backgrounds.pop(0)
                    self.backgrounds.append([self.bg] * self.cols)
                    self.y -= 1
                if char == "@":
                    if self.backgrounds[self.y][self.x] == 4:
                        raise AssertionError("hero entered a wall")
                    if self.screen[self.y][self.x] == "G":
                        self.goals += 1
                    if self.steps[-1]:
                        px, py = self.steps[-1][-1]
                        if abs(px - self.x) + abs(py - self.y) != 1:
                            raise AssertionError("hero skipped a cell")
                    self.steps[-1].append((self.x, self.y))
                self.screen[self.y][self.x] = char
                self.backgrounds[self.y][self.x] = self.bg
                self.x += 1
            if self.y == self.rows:
                self.screen.pop(0)
                self.screen.append([" "] * self.cols)
                self.backgrounds.pop(0)
                self.backgrounds.append([self.bg] * self.cols)
                self.y -= 1


def load_demo(terminal):
    module = types.ModuleType("pocketgen")
    for name in ("cls", "color", "locate", "size", "sleep_ms"):
        setattr(module, name, getattr(terminal, name))
    source = SOURCE.read_text()
    if not source.endswith(ENTRY):
        raise AssertionError("demo entry point changed")
    previous = sys.modules.get("pocketgen")
    sys.modules["pocketgen"] = module
    try:
        namespace = {"print": terminal.print}
        exec(compile(source[: -len(ENTRY)], str(SOURCE), "exec"), namespace)
    finally:
        if previous is None:
            del sys.modules["pocketgen"]
        else:
            sys.modules["pocketgen"] = previous
    return namespace


class MazeDemoTests(unittest.TestCase):
    def test_maze_walls_are_solid_blue_and_paths_are_black(self):
        terminal = Terminal(20, 10)
        demo = load_demo(terminal)
        width, height, left = 9, 7, 3
        maze = demo["make_maze"](width, height, 1)
        demo["draw_maze"](maze, width, height, left)
        for y in range(height):
            for x in range(width):
                with self.subTest(x=x, y=y):
                    self.assertEqual(terminal.backgrounds[y + 1][left + x],
                                     4 if maze[y][x] else 0)
                    if maze[y][x]:
                        self.assertEqual(terminal.screen[y + 1][left + x], " ")
        self.assertEqual(terminal.screen[height - 1][left + width - 2], "G")

    def test_maze_paths_and_seed_variation(self):
        demo = load_demo(Terminal(50, 34))
        for width, height in ((5, 5), (7, 5), (9, 7), (39, 21)):
            previous = None
            for seed in range(1, 21):
                maze = demo["make_maze"](width, height, seed)
                path = list(reversed(demo["find_path"](maze, width, height)))
                self.assertEqual(path[0], (1, 1))
                self.assertEqual(path[-1], (width - 2, height - 2))
                self.assertNotEqual(maze, previous)
                previous = maze
                for x, y in path:
                    self.assertEqual(maze[y][x], 0)
                for (x1, y1), (x2, y2) in zip(path, path[1:]):
                    self.assertEqual(abs(x1 - x2) + abs(y1 - y2), 1)

    def test_two_rounds_fit_the_screen_and_reach_the_goal(self):
        for cols, rows in ((8, 7), (10, 8), (50, 34), (80, 24)):
            with self.subTest(cols=cols, rows=rows):
                terminal = Terminal(cols, rows)
                demo = load_demo(terminal)
                demo["run_demo"](2, 0)
                self.assertEqual(len(terminal.steps), 2)
                self.assertEqual(terminal.goals, 2)
                self.assertTrue(all(steps for steps in terminal.steps))
                self.assertFalse(terminal.waits)

    def test_small_screen_exits_without_locating_outside_it(self):
        for cols, rows in ((1, 1), (7, 7), (8, 6)):
            with self.subTest(cols=cols, rows=rows):
                terminal = Terminal(cols, rows)
                load_demo(terminal)["run_demo"](1, 0)
                self.assertEqual(terminal.goals, 0)


if __name__ == "__main__":
    unittest.main()
