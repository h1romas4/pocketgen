import math
from array import array
from pocketgen import graphics_clear, graphics_line, graphics_pixel, graphics_size, refresh


_COLORS = [0x07FF, 0xFFE0, 0xF81F, 0x07E0, 0xFD20, 0xFFFF]
_FONT = {
    "0": (7, 5, 5, 5, 7), "1": (2, 6, 2, 2, 7),
    "2": (7, 1, 7, 4, 7), "3": (7, 1, 7, 1, 7),
    "4": (5, 5, 7, 1, 1), "5": (7, 4, 7, 1, 7),
    "6": (7, 4, 7, 5, 7), "7": (7, 1, 2, 2, 2),
    "8": (7, 5, 7, 5, 7), "9": (7, 5, 7, 1, 7),
    "-": (0, 0, 7, 0, 0), "+": (0, 2, 7, 2, 0),
    ".": (0, 0, 0, 0, 2), "e": (0, 7, 6, 4, 7),
    " ": (0, 0, 0, 0, 0), "/": (1, 1, 2, 4, 4),
    "%": (5, 1, 2, 4, 5), "_": (0, 0, 0, 0, 7),
    ":": (0, 2, 0, 2, 0),
    "A": (2, 5, 7, 5, 5), "B": (6, 5, 6, 5, 6),
    "C": (3, 4, 4, 4, 3), "D": (6, 5, 5, 5, 6),
    "E": (7, 4, 6, 4, 7), "F": (7, 4, 6, 4, 4),
    "G": (3, 4, 5, 5, 3), "H": (5, 5, 7, 5, 5),
    "I": (7, 2, 2, 2, 7), "J": (1, 1, 1, 5, 2),
    "K": (5, 5, 6, 5, 5), "L": (4, 4, 4, 4, 7),
    "M": (5, 7, 7, 5, 5), "N": (5, 7, 7, 7, 5),
    "O": (2, 5, 5, 5, 2), "P": (6, 5, 6, 4, 4),
    "Q": (2, 5, 5, 3, 1), "R": (6, 5, 6, 5, 5),
    "S": (3, 4, 2, 1, 6), "T": (7, 2, 2, 2, 2),
    "U": (5, 5, 5, 5, 7), "V": (5, 5, 5, 5, 2),
    "W": (5, 5, 7, 7, 5), "X": (5, 5, 2, 5, 5),
    "Y": (5, 5, 2, 2, 2), "Z": (7, 1, 2, 4, 7),
}


def _label(x, y, text, color):
    for char in text:
        rows = _FONT[char]
        for row in range(5):
            for col in range(3):
                if rows[row] & (4 >> col):
                    for dy in (0, 1):
                        for dx in (0, 1):
                            graphics_pixel(x + col * 2 + dx, y + row * 2 + dy, color)
        x += 8


def _number(value):
    return "{:.2g}".format(value)


def _finite_range(low, high):
    low, high = float(low), float(high)
    if not math.isfinite(low) or not math.isfinite(high) or low >= high:
        raise ValueError("range must be finite and increasing")
    if not math.isfinite(high - low):
        raise ValueError("range is too large")
    return low, high


def _layout(xmin, xmax, show_x_labels=True):
    width, height = graphics_size()
    if width < 128 or height < 48:
        raise ValueError("graphics area is too small for a plot")
    left, top, right, bottom = 76, 11, width - 2, height - 14
    columns = right - left + 1
    if show_x_labels and (len(_number(xmin)) + len(_number(xmax))) * 8 > columns:
        raise ValueError("graphics area is too small for axis labels")
    return left, top, right, bottom


def _axes(left, top, right, bottom, xmin, xmax, ymin, ymax, grid, show_x_labels=True):
    graphics_clear()
    if grid:
        for step in range(5):
            x = left + (right - left) * step // 4
            y = top + (bottom - top) * step // 4
            graphics_line(x, top, x, bottom, 0x2104)
            graphics_line(left, y, right, y, 0x2104)
    if xmin <= 0 <= xmax:
        x = left + int((0 - xmin) / (xmax - xmin) * (right - left))
        graphics_line(x, top, x, bottom, 0x7BEF)
    if ymin <= 0 <= ymax:
        y = bottom - int((0 - ymin) / (ymax - ymin) * (bottom - top))
        graphics_line(left, y, right, y, 0x7BEF)

    _label(2, top, _number(ymax), 0xFFFF)
    _label(2, bottom - 9, _number(ymin), 0xFFFF)
    if show_x_labels:
        _label(left, bottom + 3, _number(xmin), 0xFFFF)
        label = _number(xmax)
        _label(right - len(label) * 8 + 2, bottom + 3, label, 0xFFFF)


def _curve(values, left, top, bottom, ymin, ymax, color):
    previous = None
    for column in range(len(values)):
        value = values[column]
        if not math.isfinite(value):
            value = None
        if value is not None and previous is not None:
            if not ((value < ymin and previous < ymin) or
                    (value > ymax and previous > ymax) or
                    (value < ymin and previous > ymax) or
                    (value > ymax and previous < ymin)):
                y0 = ymin if previous < ymin else ymax if previous > ymax else previous
                y1 = ymin if value < ymin else ymax if value > ymax else value
                py0 = bottom - int((y0 - ymin) / (ymax - ymin) * (bottom - top))
                py1 = bottom - int((y1 - ymin) / (ymax - ymin) * (bottom - top))
                graphics_line(left + column - 1, py0, left + column, py1, color)
        if value is not None and ymin <= value <= ymax:
            py = bottom - int((value - ymin) / (ymax - ymin) * (bottom - top))
            graphics_pixel(left + column, py, color)
        previous = value


def plot_position(x, y, bounds):
    xmin, xmax, ymin, ymax = bounds
    xmin, xmax = _finite_range(xmin, xmax)
    ymin, ymax = _finite_range(ymin, ymax)
    x, y = float(x), float(y)
    if not math.isfinite(x) or not math.isfinite(y) or not (xmin <= x <= xmax and ymin <= y <= ymax):
        raise ValueError("point must be finite and within plot bounds")
    left, top, right, bottom = _layout(xmin, xmax)
    return (left + int((x - xmin) / (xmax - xmin) * (right - left)),
            bottom - int((y - ymin) / (ymax - ymin) * (bottom - top)))


def plot(functions, xmin=-10, xmax=10, ymin=None, ymax=None, colors=None, grid=True):
    if isinstance(functions, (tuple, list)):
        functions = tuple(functions)
    else:
        functions = (functions,)
    if not functions or not all(callable(func) for func in functions):
        raise TypeError("expected a function or a non-empty list of functions")
    xmin, xmax = _finite_range(xmin, xmax)
    if (ymin is None) != (ymax is None):
        raise ValueError("specify both ymin and ymax")
    automatic = ymin is None
    if not automatic:
        ymin, ymax = _finite_range(ymin, ymax)

    if colors is None:
        colors = tuple(_COLORS[index % len(_COLORS)] for index in range(len(functions)))
    elif isinstance(colors, int):
        colors = (colors,)
    else:
        colors = tuple(colors)
    if len(colors) != len(functions):
        raise ValueError("provide one color per function")
    for color in colors:
        if not isinstance(color, int) or color < 0 or color > 65535:
            raise ValueError("colors must be RGB565 integers")

    left, top, right, bottom = _layout(xmin, xmax)
    columns = right - left + 1
    series = []
    low = high = None
    for func in functions:
        values = array("d")
        for column in range(columns):
            x = xmin + (xmax - xmin) * (column / (columns - 1))
            try:
                y = func(x)
            except (ValueError, ZeroDivisionError, OverflowError):
                y = None
            if y is not None:
                y = float(y)
                if not math.isfinite(y):
                    y = None
            if y is not None:
                if low is None or y < low:
                    low = y
                if high is None or y > high:
                    high = y
            values.append(float("nan") if y is None else y)
        series.append(values)
    if low is None:
        raise ValueError("no finite samples")
    if automatic:
        if low == high:
            margin = abs(low) * 0.1
            if margin < 1.0:
                margin = 1.0
            ymin, ymax = _finite_range(low - margin, high + margin)
        else:
            ymin, ymax = _finite_range(low, high)

    _axes(left, top, right, bottom, xmin, xmax, ymin, ymax, grid)
    for index in range(len(functions)):
        _curve(series[index], left, top, bottom, ymin, ymax, colors[index])
    refresh()
    return xmin, xmax, ymin, ymax
