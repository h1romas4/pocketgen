from pocketgen import cls, color, locate, size, sleep_ms


SIN = (0, 25, 49, 71, 91, 106, 118, 125,
       128, 125, 118, 106, 91, 71, 49, 25,
       0, -25, -49, -71, -91, -106, -118, -125,
       -128, -125, -118, -106, -91, -71, -49, -25)

VERTICES = ((-128, -128, -128), (128, -128, -128),
            (128, 128, -128), (-128, 128, -128),
            (-128, -128, 128), (128, -128, 128),
            (128, 128, 128), (-128, 128, 128))

EDGES = ((0, 1), (1, 2), (2, 3), (3, 0),
         (4, 5), (5, 6), (6, 7), (7, 4),
         (0, 4), (1, 5), (2, 6), (3, 7))


def write_label(text, cols):
    index = 0
    while index < len(text) and index < cols - 1:
        print(text[index], end="")
        index += 1


def project(frame, width, height, scale):
    pitch = (frame // 2) % 32
    yaw = frame % 32
    sx, cx = SIN[pitch], SIN[(pitch + 8) % 32]
    sy, cy = SIN[yaw], SIN[(yaw + 8) % 32]
    points = []
    for x, y, z in VERTICES:
        rotated_y = (y * cx - z * sx) // 128
        rotated_z = (y * sx + z * cx) // 128
        rotated_x = (x * cy + rotated_z * sy) // 128
        depth = (rotated_z * cy - x * sy) // 128
        distance = 768 + depth
        px = width // 2 + rotated_x * scale * 2 * 768 // (128 * distance)
        py = height // 2 + rotated_y * scale * 768 // (128 * distance)
        points.append((px, py))
    return points


def line(canvas, start, end):
    x, y = start
    end_x, end_y = end
    dx = abs(end_x - x)
    dy = abs(end_y - y)
    step_x = 1 if x < end_x else -1
    step_y = 1 if y < end_y else -1
    error = dx - dy
    if dx > 2 * dy:
        glyph = "-"
    elif dy > 2 * dx:
        glyph = "|"
    elif step_x == step_y:
        glyph = "\\"
    else:
        glyph = "/"
    while True:
        if 0 <= y < len(canvas) and 0 <= x < len(canvas[0]):
            canvas[y][x] = glyph
        if x == end_x and y == end_y:
            break
        twice = 2 * error
        if twice > -dy:
            error -= dy
            x += step_x
        if twice < dx:
            error += dx
            y += step_y


def run_demo(frames=0, delay=80):
    cols, rows = size()
    cls()
    if cols < 24 or rows < 12:
        write_label("Screen too small", cols)
        return
    width = cols - 1 if cols - 1 < 49 else 49
    height = rows - 2 if rows - 2 < 27 else 27
    left = (cols - width) // 2
    top = (rows - height) // 2
    scale = (width - 1) * 128 // 1200
    height_scale = (height - 1) * 128 // 600
    if height_scale < scale:
        scale = height_scale
    if scale > 10:
        scale = 10

    color(14)
    locate(0, 0)
    write_label(" 3D CUBE // POCKETGEN", cols)
    frame = 0
    while not frames or frame < frames:
        canvas = [[" "] * width for _ in range(height)]
        points = project(frame, width, height, scale)
        for start, end in EDGES:
            line(canvas, points[start], points[end])
        for x, y in points:
            if 0 <= x < width and 0 <= y < height:
                canvas[y][x] = "+"
        color(11)
        for y in range(height):
            locate(left, top + y)
            print("".join(canvas[y]), end="")
        frame += 1
        if delay:
            sleep_ms(delay)


run_demo()
