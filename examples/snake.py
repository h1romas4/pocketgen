from pocketgen import (
    cls, getkey, graphics_clear, graphics_line, graphics_show,
    graphics_size, graphics_tile, sleep_ms,
)


BACKGROUND = 0x0862
PALETTE = {".": BACKGROUND, "g": 0x07E0, "G": 0x05A0,
           "w": 0xFFFF, "k": 0x0000, "r": 0xF800,
           "R": 0xA800, "y": 0xFFE0}
HEAD = ("..gggg..", ".gggggg.", "ggwggwgg", "ggkggkgg",
        "gggggggg", "ggGggGgg", ".ggGGgg.", "..gggg..")
BODY = ("..GGGG..", ".GggggG.", "GggggggG", "GggGGggG",
        "GggGGggG", "GggggggG", ".GggggG.", "..GGGG..")
FOOD = ("...gg...", "....g...", "..rrrr..", ".rryrrr.",
        ".rrrrrr.", ".rrrrrr.", "..RRRR..", "........")
EMPTY = ("........",) * 8
FONT = {
    " ": ("...", "...", "...", "...", "..."),
    "0": ("ggg", "g.g", "g.g", "g.g", "ggg"),
    "1": (".g.", "gg.", ".g.", ".g.", "ggg"),
    "2": ("ggg", "..g", "ggg", "g..", "ggg"),
    "3": ("ggg", "..g", "ggg", "..g", "ggg"),
    "4": ("g.g", "g.g", "ggg", "..g", "..g"),
    "5": ("ggg", "g..", "ggg", "..g", "ggg"),
    "6": ("ggg", "g..", "ggg", "g.g", "ggg"),
    "7": ("ggg", "..g", "..g", "..g", "..g"),
    "8": ("ggg", "g.g", "ggg", "g.g", "ggg"),
    "9": ("ggg", "g.g", "ggg", "..g", "ggg"),
    "A": (".g.", "g.g", "ggg", "g.g", "g.g"),
    "C": ("ggg", "g..", "g..", "g..", "ggg"),
    "E": ("ggg", "g..", "gg.", "g..", "ggg"),
    "G": ("ggg", "g..", "g.g", "g.g", "ggg"),
    "I": ("ggg", ".g.", ".g.", ".g.", "ggg"),
    "M": ("g.g", "ggg", "ggg", "g.g", "g.g"),
    "N": ("g.g", "ggg", "ggg", "ggg", "g.g"),
    "O": ("ggg", "g.g", "g.g", "g.g", "ggg"),
    "R": ("gg.", "g.g", "gg.", "g.g", "g.g"),
    "S": ("ggg", "g..", "ggg", "..g", "ggg"),
    "U": ("g.g", "g.g", "g.g", "g.g", "ggg"),
    "V": ("g.g", "g.g", "g.g", "g.g", ".g."),
    "W": ("g.g", "g.g", "ggg", "ggg", "g.g"),
    "Y": ("g.g", "g.g", ".g.", ".g.", ".g."),
}


def new_food(snake, width, height, state):
    free = width * height - len(snake)
    if not free:
        return None, state
    state = (state * 109 + 1021) % 32768
    selected = state % free
    for y in range(height):
        for x in range(width):
            if (x, y) not in snake:
                if selected == 0:
                    return (x, y), state
                selected -= 1
    raise RuntimeError("food placement failed")


def run_demo(frames=0, delay=130):
    screen_width, screen_height = graphics_size()
    width = (screen_width - 4) // 16
    height = (screen_height - 54) // 16
    if width < 12 or height < 8:
        raise ValueError("graphics area is too small")
    if width > 26:
        width = 26
    if height > 22:
        height = 22
    left = (screen_width - width * 16) // 2
    top = 28
    snake = [(width // 2, height // 2),
             (width // 2 - 1, height // 2),
             (width // 2 - 2, height // 2)]
    direction = (1, 0)
    score = 0
    food, state = new_food(snake, width, height, 1)
    pending = getkey()
    cls()
    print("Snake: arrows / WASD, Q / Esc to quit")
    graphics_clear(BACKGROUND)

    def draw(x, y, tile):
        graphics_tile(left + x * 16, top + y * 16, tile, PALETTE, 2)

    def label(text, x, y, color):
        palette = {".": BACKGROUND, "g": color}
        for char in text:
            graphics_tile(x, y, FONT[char], palette, 2)
            x += 8

    def border(color):
        x0, y0 = left - 2, top - 2
        x1, y1 = left + width * 16 + 1, top + height * 16 + 1
        graphics_line(x0, y0, x1, y0, color)
        graphics_line(x0, y1, x1, y1, color)
        graphics_line(x0, y0, x0, y1, color)
        graphics_line(x1, y0, x1, y1, color)

    def header():
        label("SCORE " + str(score), left, 8, 0xFFFF)

    header()
    border(0x5D1F)
    for index in range(len(snake)):
        x, y = snake[index]
        draw(x, y, HEAD if index == 0 else BODY)
    draw(food[0], food[1], FOOD)

    frame = 0
    while not frames or frame < frames:
        changed = False
        key = pending
        pending = None
        while True:
            if key is None:
                key = getkey()
            if key is None:
                break
            if key in ("q", "Q", "ESC"):
                return
            if key in ("UP", "w", "W"):
                candidate = (0, -1)
            elif key in ("DOWN", "s", "S"):
                candidate = (0, 1)
            elif key in ("LEFT", "a", "A"):
                candidate = (-1, 0)
            elif key in ("RIGHT", "d", "D"):
                candidate = (1, 0)
            else:
                candidate = None
            if candidate is not None and not changed and candidate != (-direction[0], -direction[1]):
                direction = candidate
                changed = True
            key = None

        old_head = snake[0]
        next_head = (old_head[0] + direction[0], old_head[1] + direction[1])
        eating = next_head == food
        if (next_head[0] < 0 or next_head[0] >= width or
                next_head[1] < 0 or next_head[1] >= height or
                (next_head in snake and (eating or next_head != snake[-1]))):
            border(0xF800)
            label("GAME OVER", left, top + height * 16 + 6, 0xF800)
            print("Game over. Score:", score)
            graphics_show()
            return
        if not eating:
            tail = snake.pop()
            draw(tail[0], tail[1], EMPTY)
        draw(old_head[0], old_head[1], BODY)
        snake.insert(0, next_head)
        draw(next_head[0], next_head[1], HEAD)
        if eating:
            score += 1
            header()
            food, state = new_food(snake, width, height, state)
            if food is None:
                border(0xFFE0)
                label("YOU WIN", left, top + height * 16 + 6, 0xFFE0)
                print("You win! Score:", score)
                graphics_show()
                return
            draw(food[0], food[1], FOOD)
        frame += 1
        if delay:
            sleep_ms(delay)


run_demo()
