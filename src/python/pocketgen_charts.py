import math
from array import array
from pocketgen import graphics_clear, graphics_line, graphics_pixel, graphics_size, refresh
from pocketgen_plot import _COLORS, _FONT, _axes, _curve, _finite_range, _label, _layout, _number


def _value(value):
    value = float(value)
    if not math.isfinite(value):
        raise ValueError("data must be finite")
    return value


def _length(values):
    count = len(values)
    if count == 0:
        raise ValueError("data must not be empty")
    return count


def _range(low, high):
    if low == high:
        margin = abs(low) * 0.1
        if margin < 1:
            margin = 1.0
        low, high = low - margin, high + margin
    return _finite_range(low, high)


def _pair_range(low, high, auto_low, auto_high):
    if (low is None) != (high is None):
        raise ValueError("specify both range endpoints")
    return _range(auto_low, auto_high) if low is None else _finite_range(low, high)


def _color(color):
    if not isinstance(color, int) or color < 0 or color > 65535:
        raise ValueError("color must be an RGB565 integer")
    return color


def _labels(labels, count):
    if labels is None:
        labels = [str(index + 1) for index in range(count)]
    if len(labels) != count:
        raise ValueError("one label is required per value")
    result = []
    for label in labels:
        if not isinstance(label, str):
            raise TypeError("labels must be strings")
        label = label.upper()
        if not label or any(char not in _FONT or char == "e" for char in label):
            raise ValueError("unsupported or empty label")
        result.append(label)
    return result


def linear_fit(x, y):
    count = _length(x)
    if count != len(y) or count < 2:
        raise ValueError("regression requires equally sized data with at least two points")
    mean_x = mean_y = sxx = syy = sxy = 0.0
    for index in range(count):
        px, py = _value(x[index]), _value(y[index])
        dx, dy = px - mean_x, py - mean_y
        mean_x += dx / (index + 1)
        mean_y += dy / (index + 1)
        sxx += dx * (px - mean_x)
        syy += dy * (py - mean_y)
        sxy += dx * (py - mean_y)
        if not all(math.isfinite(v) for v in (mean_x, mean_y, sxx, syy, sxy)):
            raise ValueError("regression exceeds numeric range")
    if sxx <= 0:
        raise ValueError("x values must vary")
    slope = sxy / sxx
    intercept = mean_y - slope * mean_x
    if not math.isfinite(slope) or not math.isfinite(intercept):
        raise ValueError("regression exceeds numeric range")
    if syy == 0:
        r2 = None
    else:
        correlation = (sxy / math.sqrt(sxx)) / math.sqrt(syy)
        r2 = correlation * correlation
        if not math.isfinite(r2):
            raise ValueError("regression exceeds numeric range")
        if r2 > 1:
            r2 = 1.0
    return slope, intercept, r2


def scatter(x, y, fit=None, xmin=None, xmax=None, ymin=None, ymax=None,
            color=0x07FF, fit_color=0xFFE0, grid=True):
    count = _length(x)
    if count != len(y):
        raise ValueError("x and y must have the same length")
    if fit is not None and not callable(fit):
        raise TypeError("fit must be callable")
    _color(color)
    _color(fit_color)
    low_x = high_x = _value(x[0])
    low_y = high_y = _value(y[0])
    for index in range(1, count):
        px, py = _value(x[index]), _value(y[index])
        if px < low_x:
            low_x = px
        if px > high_x:
            high_x = px
        if py < low_y:
            low_y = py
        if py > high_y:
            high_y = py
    xmin, xmax = _pair_range(xmin, xmax, low_x, high_x)
    left, top, right, bottom = _layout(xmin, xmax)
    columns = right - left + 1
    curve = None
    if fit is not None:
        curve = array("d")
        for column in range(columns):
            px = xmin + (xmax - xmin) * (column / (columns - 1))
            try:
                py = fit(px)
            except (ValueError, ZeroDivisionError, OverflowError):
                py = None
            if py is not None:
                py = float(py)
                if not math.isfinite(py):
                    py = None
            if py is not None:
                if py < low_y:
                    low_y = py
                if py > high_y:
                    high_y = py
            curve.append(float("nan") if py is None else py)
    ymin, ymax = _pair_range(ymin, ymax, low_y, high_y)
    _axes(left, top, right, bottom, xmin, xmax, ymin, ymax, grid)
    if curve is not None:
        _curve(curve, left, top, bottom, ymin, ymax, fit_color)
    for index in range(count):
        px, py = _value(x[index]), _value(y[index])
        if xmin <= px <= xmax and ymin <= py <= ymax:
            sx = left + int((px - xmin) / (xmax - xmin) * (right - left))
            sy = bottom - int((py - ymin) / (ymax - ymin) * (bottom - top))
            graphics_pixel(sx, sy, color)
            if sx > left:
                graphics_pixel(sx - 1, sy, color)
            if sx < right:
                graphics_pixel(sx + 1, sy, color)
            if sy > top:
                graphics_pixel(sx, sy - 1, color)
            if sy < bottom:
                graphics_pixel(sx, sy + 1, color)
    refresh()
    return xmin, xmax, ymin, ymax


def _draw_bars(values, left, top, right, bottom, ymin, ymax, color):
    count = len(values)
    baseline = bottom - int((0 - ymin) / (ymax - ymin) * (bottom - top))
    for index in range(count):
        value = _value(values[index])
        clipped = ymin if value < ymin else ymax if value > ymax else value
        if clipped == 0:
            continue
        y = bottom - int((clipped - ymin) / (ymax - ymin) * (bottom - top))
        start = left + (right - left + 1) * index // count
        end = left + (right - left + 1) * (index + 1) // count - 1
        if end - start >= 2:
            start += 1
            end -= 1
        for column in range(start, end + 1):
            graphics_line(column, baseline, column, y, color)


def hist(data, bins=10, xmin=None, xmax=None, color=0x07FF, grid=True):
    count = _length(data)
    if not isinstance(bins, int) or bins < 1:
        raise ValueError("bins must be a positive integer")
    _color(color)
    low = high = _value(data[0])
    for index in range(1, count):
        value = _value(data[index])
        if value < low:
            low = value
        if value > high:
            high = value
    xmin, xmax = _pair_range(xmin, xmax, low, high)
    if low < xmin or high > xmax:
        raise ValueError("data outside histogram range")
    left, top, right, bottom = _layout(xmin, xmax)
    if bins > right - left + 1:
        raise ValueError("too many bins for the graphics area")
    edges = [xmin + (xmax - xmin) * (index / bins) for index in range(bins)] + [xmax]
    for index in range(bins):
        if edges[index] >= edges[index + 1]:
            raise ValueError("bin edges cannot be represented")
    counts = [0] * bins
    for index in range(count):
        value = _value(data[index])
        bucket = int((value - xmin) / (xmax - xmin) * bins)
        if bucket == bins:
            bucket -= 1
        while bucket > 0 and value < edges[bucket]:
            bucket -= 1
        while bucket + 1 < bins and value >= edges[bucket + 1]:
            bucket += 1
        counts[bucket] += 1
    maximum = 0
    for value in counts:
        if value > maximum:
            maximum = value
    ymin, ymax = 0.0, float(maximum)
    _axes(left, top, right, bottom, xmin, xmax, ymin, ymax, grid)
    _draw_bars(counts, left, top, right, bottom, ymin, ymax, color)
    refresh()
    return counts, edges


def bar(values, labels=None, ymin=None, ymax=None, color=0x07FF, grid=True):
    count = _length(values)
    _color(color)
    labels = _labels(labels, count)
    low = high = 0.0
    for index in range(count):
        value = _value(values[index])
        if value < low:
            low = value
        if value > high:
            high = value
    ymin, ymax = _pair_range(ymin, ymax, low, high)
    if ymin > 0 or ymax < 0:
        raise ValueError("bar range must include zero")
    left, top, right, bottom = _layout(0, count, False)
    columns = right - left + 1
    if count > columns:
        raise ValueError("too many bars for the graphics area")
    for index in range(count):
        start = left + columns * index // count
        end = left + columns * (index + 1) // count
        if len(labels[index]) * 8 > end - start:
            raise ValueError("bar label does not fit")
    _axes(left, top, right, bottom, 0, count, ymin, ymax, grid, False)
    _draw_bars(values, left, top, right, bottom, ymin, ymax, color)
    for index in range(count):
        start = left + columns * index // count
        end = left + columns * (index + 1) // count
        _label(start + (end - start - len(labels[index]) * 8) // 2,
               bottom + 3, labels[index], 0xFFFF)
    refresh()
    return ymin, ymax


def pie(values, labels=None, colors=None):
    count = _length(values)
    labels = _labels(labels, count)
    if colors is None:
        colors = [_COLORS[index % len(_COLORS)] for index in range(count)]
    if len(colors) != count:
        raise ValueError("one color is required per value")
    for color in colors:
        _color(color)
    maximum = 0.0
    for index in range(count):
        value = _value(values[index])
        if value < 0:
            raise ValueError("pie values must be non-negative")
        if value > maximum:
            maximum = value
    if maximum == 0:
        raise ValueError("pie values must have a positive sum")
    total = 0.0
    for index in range(count):
        total += _value(values[index]) / maximum
    fractions = [_value(values[index]) / maximum / total for index in range(count)]
    legend = [labels[index] + " " + _number(fractions[index] * 100) + "%"
              for index in range(count)]
    legend_width = 0
    for text in legend:
        if len(text) * 8 > legend_width:
            legend_width = len(text) * 8
    width, height = graphics_size()
    radius = (width - legend_width - 24) // 2
    if radius > (height - 4) // 2:
        radius = (height - 4) // 2
    if radius < 12 or count * 12 > height:
        raise ValueError("graphics area is too small for pie chart and labels")
    cx, cy = radius + 1, height // 2
    legend_x = 2 * radius + 5
    angles = [0.0]
    for fraction in fractions:
        angles.append(angles[-1] + fraction * 2 * math.pi)
    angles[-1] = 2 * math.pi
    boundaries = [(math.cos(angles[index]), math.sin(angles[index]))
                  for index in range(1, count)]
    graphics_clear()
    for row in range(cy - radius, cy + radius + 1):
        dy = cy - row
        span = int(math.sqrt(radius * radius - dy * dy))
        lo, hi = cx - span, cx + span
        cuts = [lo, hi + 1]
        if dy == 0 and lo < cx + 1 <= hi:
            cuts.append(cx + 1)
        else:
            for cosine, sine in boundaries:
                if dy * sine > 0:
                    crossing = cx + dy * cosine / sine
                    if lo < crossing <= hi:
                        cuts.append(int(math.ceil(crossing)))
        cuts.sort()
        for index in range(len(cuts) - 1):
            start, end = cuts[index], cuts[index + 1] - 1
            if start > end:
                continue
            angle = math.atan2(dy, (start + end) / 2 - cx)
            if angle < 0:
                angle += 2 * math.pi
            sector = 0
            while sector < count - 1 and angle >= angles[sector + 1]:
                sector += 1
            graphics_line(start, row, end, row, colors[sector])
    for index in range(count):
        y = index * 12
        for row in range(y, y + 10):
            graphics_line(legend_x, row, legend_x + 9, row, colors[index])
        _label(legend_x + 14, y, legend[index], 0xFFFF)
    refresh()
    return fractions
