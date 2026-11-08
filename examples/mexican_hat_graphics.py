import math
from pocketgen import (
    graphics_clear, graphics_pixel, graphics_show, graphics_size, refresh,
    ticks_diff, ticks_ms,
)


COLORS = (0x001F, 0xF800, 0xF81F, 0x07E0, 0x07FF, 0xFFE0, 0xFFFF)


def surface(x, depth):
    # Keep the photographed program's degree-to-radian approximation.
    radius = math.sqrt(x * x + depth * depth) * (3.14 / 180)
    return 100 * math.cos(radius) - 30 * math.cos(3 * radius)


def project(x, depth, z, width, height):
    screen_x = (80 + x / 3 - depth / 6) * (width - 1) / 159
    screen_y = (40 - depth / 6 - z / 4) * (height - 1) / 99
    return math.floor(screen_x + 0.5), math.floor(screen_y + 0.5)


def plot_line(horizon, height, left, top, start, end, color):
    x, y = start
    end_x, end_y = end
    dx = abs(end_x - x)
    dy = abs(end_y - y)
    step_x = 1 if x < end_x else -1
    step_y = 1 if y < end_y else -1
    error = dx - dy
    while True:
        if 0 <= x < len(horizon) and 0 <= y < height and y <= horizon[x]:
            graphics_pixel(left + x, top + y, color)
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


def run_demo(slices=0):
    width, height = graphics_size()
    if width < 24 or height < 12:
        raise ValueError("graphics area is too small")
    start = ticks_ms()
    graphics_clear()
    graph_width = height * 160 // 100
    if graph_width > width:
        graph_width = width
    graph_height = graph_width * 100 // 160
    left = (width - graph_width) // 2
    top = (height - graph_height) // 2
    horizon = [graph_height] * graph_width
    for depth in range(-180, 181, 6):
        previous = None
        for x in range(-180, 181, 4):
            z = surface(x, depth)
            point = project(x, depth, z, graph_width, graph_height)
            color = COLORS[math.floor((z + 100) * 0.035)]
            if previous is not None:
                plot_line(horizon, graph_height, left, top, previous, point, color)
            previous = point
        completed = (depth + 180) // 6 + 1
        done = depth == 180 or (slices and completed >= slices)
        if done:
            break
    refresh()
    elapsed = ticks_diff(ticks_ms(), start)
    print("Mexican Hat: {} ms".format(elapsed))
    graphics_show()


run_demo()
