from pocketgen import (
    cls, getkey, graphics_clear, graphics_line, graphics_pixel, graphics_show,
    graphics_size, graphics_tile, sleep_ms, ticks_diff, ticks_ms,
)


SIN = (0, 25, 49, 71, 91, 106, 118, 125,
       128, 125, 118, 106, 91, 71, 49, 25,
       0, -25, -49, -71, -91, -106, -118, -125,
       -128, -125, -118, -106, -91, -71, -49, -25)
COLORS = (0xF81F, 0xF800, 0xFD20, 0xFFE0, 0x07E0,
          0x07FF, 0x5D1F, 0xFFFF)
FONT = {
    " ": ("...", "...", "...", "...", "..."),
    "A": (".#.", "#.#", "###", "#.#", "#.#"),
    "B": ("##.", "#.#", "##.", "#.#", "##."),
    "C": ("###", "#..", "#..", "#..", "###"),
    "D": ("##.", "#.#", "#.#", "#.#", "##."),
    "E": ("###", "#..", "##.", "#..", "###"),
    "F": ("###", "#..", "##.", "#..", "#.."),
    "G": ("###", "#..", "#.#", "#.#", "###"),
    "I": ("###", ".#.", ".#.", ".#.", "###"),
    "K": ("#.#", "#.#", "##.", "#.#", "#.#"),
    "N": ("#.#", "###", "###", "###", "#.#"),
    "O": ("###", "#.#", "#.#", "#.#", "###"),
    "P": ("###", "#.#", "###", "#..", "#.."),
    "Q": ("###", "#.#", "#.#", "###", "..#"),
    "R": ("##.", "#.#", "##.", "#.#", "#.#"),
    "S": ("###", "#..", "###", "..#", "###"),
    "T": ("###", ".#.", ".#.", ".#.", ".#."),
    "U": ("#.#", "#.#", "#.#", "#.#", "###"),
    "V": ("#.#", "#.#", "#.#", "#.#", ".#."),
    "W": ("#.#", "#.#", "###", "###", "#.#"),
    "X": ("#.#", "#.#", ".#.", "#.#", "#.#"),
}
SCENES = ("WARP", "NEON GRID", "CUBE", "WAVES", "FIREWORKS")
VERTICES = ((-128, -128, -128), (128, -128, -128),
            (128, 128, -128), (-128, 128, -128),
            (-128, -128, 128), (128, -128, 128),
            (128, 128, 128), (-128, 128, 128))
EDGES = ((0, 1), (1, 2), (2, 3), (3, 0),
         (4, 5), (5, 6), (6, 7), (7, 4),
         (0, 4), (1, 5), (2, 6), (3, 7))


def random_value(state):
    return (state * 109 + 1021) % 32768


def label(text, x, y, scale, rainbow=False):
    for index in range(len(text)):
        color = COLORS[index % len(COLORS)] if rainbow else 0xFFFF
        graphics_tile(x + index * 4 * scale, y, FONT[text[index]],
                      {".": None, "#": color}, scale)


def frame_border(width, height, scene):
    graphics_clear()
    scale = 5 if width >= 200 else 3
    label("POCKETGEN", (width - 36 * scale) // 2, 8, scale, True)
    graphics_line(8, 44, width - 9, 44, 0xF81F)
    graphics_line(8, height - 33, width - 9, height - 33, 0x07FF)
    label(SCENES[scene], 10, height - 23, 2, True)
    label("SPACE NEXT Q QUIT", width - 138, height - 23, 2)


def draw_segments(segments, erase=False, bold=False):
    for x0, y0, x1, y1, color in segments:
        ink = 0 if erase else color
        graphics_line(x0, y0, x1, y1, ink)
        if bold:
            graphics_line(x0 + 1, y0, x1 + 1, y1, ink)


def background_stars(width, top, bottom):
    seed = 7
    for _ in range(90):
        seed = random_value(seed)
        x = 8 + seed % (width - 16)
        seed = random_value(seed)
        y = top + seed % (bottom - top)
        graphics_pixel(x, y, 0x31AE)


def warp(frame, width, top, bottom, state):
    stars, seed = state
    cx, cy = width // 2, (top + bottom) // 2
    height = bottom - top
    segments = []
    for star in stars:
        x, y, depth = star
        next_depth = depth - 3
        old_x = cx + x * width // (depth * 5)
        old_y = cy + y * height // (depth * 5)
        new_x = cx + x * width // (next_depth * 5)
        new_y = cy + y * height // (next_depth * 5)
        if next_depth <= 6 or new_x < 8 or new_x >= width - 8 or new_y < top or new_y >= bottom:
            seed = random_value(seed)
            star[0] = seed % 241 - 120
            seed = random_value(seed)
            star[1] = seed % 241 - 120
            star[2] = 100
        else:
            star[2] = next_depth
            color = 0x39E7 if next_depth > 60 else (0x07FF if next_depth > 30 else 0xFFFF)
            segments.append((old_x, old_y, new_x, new_y, color))
    state[1] = seed
    return segments


def init_warp():
    stars = []
    seed = 1
    for _ in range(72):
        seed = random_value(seed)
        x = seed % 241 - 120
        seed = random_value(seed)
        y = seed % 241 - 120
        seed = random_value(seed)
        stars.append([x, y, 35 + seed % 66])
    return [stars, seed]


def neon_sun(width, top, bottom):
    horizon = top + (bottom - top) * 3 // 5
    radius = (bottom - top) // 4
    if radius > width // 4:
        radius = width // 4
    cx, cy = width // 2, horizon - radius // 3
    for dy in range(-radius, radius):
        if cy + dy >= horizon:
            break
        if dy > 0 and dy % 12 >= 7:
            continue
        dx = int((radius * radius - dy * dy) ** 0.5)
        color = 0xFFE0 if dy < -radius // 2 else (0xFD20 if dy < 0 else 0xF81F)
        graphics_line(cx - dx, cy + dy, cx + dx, cy + dy, color)
    graphics_line(8, horizon, width - 9, horizon, 0xF81F)
    return horizon


def neon_grid(frame, width, horizon, bottom):
    segments = []
    cx = width // 2
    for x in range(8, width - 8, 32):
        segments.append((cx, horizon + 1, x, bottom - 1, 0xF81F))
    depth = bottom - horizon - 2
    for index in range(1, 9):
        distance = (index * 14 + frame % 14)
        y = horizon + 1 + distance * distance * depth // (126 * 126)
        if y < bottom:
            segments.append((8, y, width - 9, y, 0x07FF))
    return segments


def cube(frame, width, top, bottom):
    cx, cy = width // 2, (top + bottom) // 2
    size = (bottom - top) // 5
    if size > width // 5:
        size = width // 5
    segments = []
    for layer in range(2):
        angle = frame if layer == 0 else -frame
        sx, cx_rot = SIN[(angle // 2) % 32], SIN[(angle // 2 + 8) % 32]
        sy, cy_rot = SIN[angle % 32], SIN[(angle + 8) % 32]
        points = []
        scale = size if layer == 0 else size // 2
        for x, y, z in VERTICES:
            ry = (y * cx_rot - z * sx) // 128
            rz = (y * sx + z * cx_rot) // 128
            rx = (x * cy_rot + rz * sy) // 128
            depth = (rz * cy_rot - x * sy) // 128
            distance = 768 + depth
            points.append((cx + rx * scale * 768 // (128 * distance),
                           cy + ry * scale * 768 // (128 * distance)))
        for index in range(len(EDGES)):
            start, end = EDGES[index]
            x0, y0 = points[start]
            x1, y1 = points[end]
            segments.append((x0, y0, x1, y1, COLORS[(index + frame // 4 + layer * 3) % 8]))
    return segments


def waves(frame, width, top, bottom):
    segments = []
    center = (top + bottom) // 2
    amplitude = (bottom - top) // 4
    for wave in range(3):
        previous = None
        for index in range(33):
            x = 8 + (width - 17) * index // 32
            phase = (index * (wave + 1) + frame * (wave + 1) + wave * 6) % 32
            y = center + SIN[phase] * amplitude // 128
            if previous is not None:
                segments.append((previous[0], previous[1], x, y, COLORS[wave * 2]))
            previous = (x, y)
    return segments


def fireworks(frame, width, top, bottom):
    segments = []
    height = bottom - top
    for burst in range(3):
        age = (frame + burst * 11) % 32
        radius = 4 + age * height // 100
        cx = width * (burst + 1) // 4
        cy = top + height * (2 + burst % 2) // 5
        limit = cx - 8
        if width - 9 - cx < limit:
            limit = width - 9 - cx
        if cy - top < limit:
            limit = cy - top
        if bottom - 1 - cy < limit:
            limit = bottom - 1 - cy
        if radius > limit:
            radius = limit
        for ray in range(0, 32, 2):
            dx, dy = SIN[(ray + 8) % 32], SIN[ray]
            inner = radius * 2 // 3
            color = COLORS[(burst * 2 + ray // 2 + frame // 8) % 8]
            segments.append((cx + dx * inner // 128, cy + dy * inner // 128,
                             cx + dx * radius // 128, cy + dy * radius // 128, color))
    return segments


def run_demo(scene_ms=8000, cycles=0, delay=35, frames_per_scene=0):
    width, height = graphics_size()
    if width < 240 or height < 220:
        raise ValueError("graphics area is too small")
    pending = getkey()
    cls()
    print("Mega Demo: Space next, Q / Esc quit, Ctrl+C stop")
    top, bottom = 54, height - 38
    scene = 0
    completed = 0
    while not cycles or completed < cycles:
        frame_border(width, height, scene)
        state = init_warp() if scene == 0 else None
        horizon = neon_sun(width, top, bottom) if scene == 1 else 0
        if scene == 2 or scene == 4:
            background_stars(width, top, bottom)
        previous = []
        frame = 0
        start = ticks_ms()
        while True:
            key = pending
            pending = None
            if key is None:
                key = getkey()
            if key in ("q", "Q", "ESC"):
                return
            if key == " " or (frames_per_scene and frame >= frames_per_scene):
                break
            if frame and ticks_diff(ticks_ms(), start) >= scene_ms:
                break
            draw_segments(previous, True, scene >= 2)
            if scene == 0:
                current = warp(frame, width, top, bottom, state)
            elif scene == 1:
                current = neon_grid(frame, width, horizon, bottom)
            elif scene == 2:
                current = cube(frame, width, top, bottom)
            elif scene == 3:
                current = waves(frame, width, top, bottom)
            else:
                current = fireworks(frame, width, top, bottom)
            draw_segments(current, False, scene >= 2)
            previous = current
            frame += 1
            if delay:
                sleep_ms(delay)
        scene += 1
        if scene == len(SCENES):
            scene = 0
            completed += 1
    graphics_show()


run_demo()
