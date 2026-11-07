from pocketgen import cls, color, locate, size, sleep_ms


def random_value(state):
    return (state * 109 + 1021) % 32768


def new_star(state, random_depth):
    state = random_value(state)
    x = state % 801 - 400
    state = random_value(state)
    y = state % 501 - 250
    state = random_value(state)
    depth = 12 + state % 53 if random_depth else 64
    return [x, y, depth], state


def write_label(text, cols):
    index = 0
    while index < len(text) and index < cols - 1:
        print(text[index], end="")
        index += 1


def run_demo(frames=0, delay=60):
    cols, rows = size()
    cls()
    if cols < 12 or rows < 6:
        write_label("Screen too small", cols)
        return

    color(14)
    locate(0, 0)
    write_label(" WARP // POCKETGEN", cols)
    center_x = (cols - 1) // 2
    center_y = rows // 2
    stars = []
    previous = []
    state = 1
    for _ in range(48):
        star, state = new_star(state, True)
        stars.append(star)

    frame = 0
    while not frames or frame < frames:
        color(7)
        for x, y in previous:
            locate(x, y)
            print(" ", end="")
        previous = []

        for star in stars:
            star[2] -= 1
            x = center_x + star[0] // star[2]
            y = center_y + star[1] // star[2]
            if star[2] <= 4 or x < 0 or x >= cols - 1 or y < 1 or y >= rows - 1:
                replacement, state = new_star(state, False)
                star[0], star[1], star[2] = replacement
                continue
            if star[2] > 44:
                color(8)
                glyph = "."
            elif star[2] > 20:
                color(11)
                glyph = "+"
            else:
                color(15)
                glyph = "*"
            locate(x, y)
            print(glyph, end="")
            previous.append((x, y))

        frame += 1
        if delay:
            sleep_ms(delay)


run_demo()
