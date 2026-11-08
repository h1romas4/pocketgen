from pocketgen import graphics_line, graphics_size


def graphics_tile(x, y, rows, palette, scale=1):
    if not isinstance(x, int) or not isinstance(y, int) or not isinstance(scale, int):
        raise TypeError("coordinates and scale must be integers")
    if scale < 1:
        raise ValueError("scale must be positive")
    if not isinstance(rows, (tuple, list)) or not rows:
        raise ValueError("tile must contain rows")
    if not isinstance(palette, dict):
        raise TypeError("palette must be a dictionary")
    if not isinstance(rows[0], str) or not rows[0]:
        raise ValueError("tile rows must be nonempty strings")
    tile_width = len(rows[0])
    for row in rows:
        if not isinstance(row, str) or len(row) != tile_width:
            raise ValueError("tile rows must have equal width")
        for symbol in row:
            if symbol not in palette:
                raise ValueError("tile symbol missing from palette")
            color = palette[symbol]
            if color is not None and (not isinstance(color, int) or color < 0 or color > 65535):
                raise ValueError("tile colors must be RGB565 integers or None")
    width, height = graphics_size()
    if x < 0 or y < 0 or x + tile_width * scale > width or y + len(rows) * scale > height:
        raise ValueError("tile outside graphics area")
    for row_index in range(len(rows)):
        row = rows[row_index]
        start = 0
        while start < tile_width:
            color = palette[row[start]]
            end = start + 1
            while end < tile_width and palette[row[end]] == color:
                end += 1
            if color is not None:
                for dy in range(scale):
                    py = y + row_index * scale + dy
                    graphics_line(x + start * scale, py, x + end * scale - 1, py, color)
            start = end
