import math
from pocketgen import graphics_clear, graphics_line, graphics_size, refresh


def run_demo():
    width, height = graphics_size()
    if width < 4 or height < 4:
        raise ValueError("graphics area is too small")

    graphics_clear()
    mid_y = height // 2
    axis = 0x7BEF
    curve = 0x07FF
    graphics_line(0, mid_y, width - 1, mid_y, axis)
    graphics_line(width // 2, 0, width // 2, height - 1, axis)

    amplitude = (height - 2) * 0.4
    previous_y = mid_y
    for x in range(width):
        angle = (x / (width - 1) * 4.0 - 2.0) * math.pi
        y = int(mid_y - math.sin(angle) * amplitude)
        if x:
            graphics_line(x - 1, previous_y, x, y, curve)
        previous_y = y
    refresh()


run_demo()
