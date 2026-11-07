from pocketgen import cls, color, locate, size, sleep_ms


# Samples of sin(r) / r, scaled by 128, at intervals of pi / 6.
HEIGHT = (128, 122, 106, 81, 53, 24, 0, -17, -26, -27, -21, -11,
          0, 9, 15, 16, 13, 7, 0, -6, -11, -12, -10, -5, 0)


def radius(x, y):
    squared = (x * x + y * y) * 256
    low, high = 0, 512
    while low + 1 < high:
        middle = (low + high) // 2
        if middle * middle <= squared:
            low = middle
        else:
            high = middle
    return low


def project(x, depth, width, height):
    r = radius(x, depth)
    index = r // 32
    fraction = r % 32
    if index + 1 < len(HEIGHT):
        z = (HEIGHT[index] * (32 - fraction) + HEIGHT[index + 1] * fraction) // 32
    else:
        z = 0
    screen_x = width // 2 + x * (width - 5) // 40 + depth * (width - 5) // 100
    screen_y = height // 2 + depth * (height - 1) // 32 - z * (height - 1) // 384
    return screen_x, screen_y


def plot_line(canvas, horizon, start, end):
    x, y = start
    end_x, end_y = end
    dx = abs(end_x - x)
    dy = abs(end_y - y)
    step_x = 1 if x < end_x else -1
    step_y = 1 if y < end_y else -1
    error = dx - dy
    while True:
        if 0 <= x < len(horizon) and 0 <= y < len(canvas):
            if y <= horizon[x]:
                canvas[y][x] = "*"
                horizon[x] = y
        if x == end_x and y == end_y:
            break
        twice = 2 * error
        if twice > -dy:
            error -= dy
            x += step_x
        if twice < dx:
            error += dx
            y += step_y


def write_label(text, cols):
    index = 0
    while index < len(text) and index < cols - 1:
        print(text[index], end="")
        index += 1


def run_demo(slices=0, delay=100):
    cols, rows = size()
    cls()
    if cols < 24 or rows < 12:
        write_label("Screen too small", cols)
        return
    width = cols - 1 if cols - 1 < 59 else 59
    height = rows - 2 if rows - 2 < 27 else 27
    left = (cols - width) // 2
    top = (rows - height) // 2
    canvas = [[" "] * width for _ in range(height)]
    horizon = [height] * width

    color(14)
    locate(0, 0)
    write_label(" MEXICAN HAT // POCKETGEN", cols)
    for depth in range(12, -13, -2):
        previous = project(-16, depth, width, height)
        for x in range(-15, 17):
            point = project(x, depth, width, height)
            plot_line(canvas, horizon, previous, point)
            previous = point
        color(11)
        for y in range(height):
            locate(left, top + y)
            print("".join(canvas[y]), end="")
        if delay:
            sleep_ms(delay)
        if slices and (12 - depth) // 2 + 1 >= slices:
            break


run_demo()
