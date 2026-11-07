from pocketgen import cls, color, locate, size, sleep_ms
import gc


def make_maze(width, height, seed):
    maze = [[1] * width for _ in range(height)]
    maze[1][1] = 0
    stack = [(1, 1)]
    state = seed
    while stack:
        x, y = stack[-1]
        choices = []
        for dx, dy in ((1, 0), (0, 1), (-1, 0), (0, -1)):
            nx, ny = x + dx * 2, y + dy * 2
            if 0 < nx < width - 1 and 0 < ny < height - 1 and maze[ny][nx]:
                choices.append((dx, dy))
        if choices:
            state = (state * 109 + 1021) % 32768
            dx, dy = choices[state % len(choices)]
            maze[y + dy][x + dx] = 0
            maze[y + dy * 2][x + dx * 2] = 0
            stack.append((x + dx * 2, y + dy * 2))
        else:
            stack.pop()
    return maze


def find_path(maze, width, height):
    previous = [[None] * width for _ in range(height)]
    previous[1][1] = (1, 1)
    queue = [(1, 1)]
    head = 0
    goal = (width - 2, height - 2)
    while head < len(queue) and previous[goal[1]][goal[0]] is None:
        x, y = queue[head]
        head += 1
        for dx, dy in ((1, 0), (0, 1), (-1, 0), (0, -1)):
            nx, ny = x + dx, y + dy
            if 0 <= nx < width and 0 <= ny < height:
                if maze[ny][nx] == 0 and previous[ny][nx] is None:
                    previous[ny][nx] = (x, y)
                    queue.append((nx, ny))
    if previous[goal[1]][goal[0]] is None:
        raise ValueError("maze has no path to the goal")
    path = []
    point = goal
    while point != (1, 1):
        path.append(point)
        point = previous[point[1]][point[0]]
    path.append((1, 1))
    return path


def draw_maze(maze, width, height, left):
    color(7)
    cls()
    color(11)
    locate(0, 0)
    print("MAZE")
    for y in range(height):
        color(15, 4)
        locate(left, y + 1)
        print(" " * width, end="")
        color(7)
        x = 0
        while x < width:
            if maze[y][x]:
                x += 1
                continue
            start = x
            while x < width and not maze[y][x]:
                x += 1
            locate(left + start, y + 1)
            print(" " * (x - start), end="")
    color(14)
    locate(left + width - 2, height - 1)
    print("G", end="")


def run_demo(rounds=0, delay=80):
    cols, rows = size()
    if cols < 8 or rows < 7:
        cls()
        print("Screen too small")
        return
    width = cols - 1 if cols - 1 < 39 else 39
    height = rows - 2 if rows - 2 < 21 else 21
    if width % 2 == 0:
        width -= 1
    if height % 2 == 0:
        height -= 1
    left = (cols - width) // 2
    completed = 0
    seed = 1
    while rounds == 0 or completed < rounds:
        maze = make_maze(width, height, seed)
        path = find_path(maze, width, height)
        draw_maze(maze, width, height, left)
        x, y = path.pop()
        color(10)
        locate(left + x, y + 1)
        print("@", end="")
        if delay:
            sleep_ms(delay)
        while path:
            color(7)
            locate(left + x, y + 1)
            print(" ", end="")
            x, y = path.pop()
            color(10)
            locate(left + x, y + 1)
            print("@", end="")
            if delay:
                sleep_ms(delay)
        color(14)
        locate(0, 0)
        print("GOAL!")
        if delay:
            sleep_ms(700)
        maze = None
        path = None
        gc.collect()
        seed = seed % 32767 + 1
        if rounds:
            completed += 1


run_demo()
