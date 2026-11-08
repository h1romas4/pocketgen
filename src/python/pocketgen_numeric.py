import math


def _finite(value):
    value = float(value)
    if not math.isfinite(value):
        raise ValueError("values must be finite")
    return value


def _function(func):
    if not callable(func):
        raise TypeError("function must be callable")


def solve(func, a, b, tol=1e-8, max_iter=100):
    _function(func)
    a, b, tol = _finite(a), _finite(b), _finite(tol)
    if a >= b or not math.isfinite(b - a):
        raise ValueError("interval must be finite and increasing")
    if tol <= 0:
        raise ValueError("tol must be positive")
    if not isinstance(max_iter, int) or max_iter < 1:
        raise ValueError("max_iter must be a positive integer")
    fa, fb = _finite(func(a)), _finite(func(b))
    if fa == 0:
        return a
    if fb == 0:
        return b
    if (fa < 0) == (fb < 0):
        raise ValueError("function values must have opposite signs")
    for _ in range(max_iter):
        middle = a + (b - a) * 0.5
        if b - a <= tol:
            return middle
        if middle == a or middle == b:
            raise ValueError("tolerance cannot be reached at this precision")
        fm = _finite(func(middle))
        if fm == 0:
            return middle
        if (fm < 0) == (fa < 0):
            a, fa = middle, fm
        else:
            b = middle
    if b - a <= tol:
        return a + (b - a) * 0.5
    raise ValueError("maximum iterations exceeded")


def integrate(func, a, b, steps=100):
    _function(func)
    a, b = _finite(a), _finite(b)
    if not isinstance(steps, int) or steps < 2 or steps % 2:
        raise ValueError("steps must be an even integer of at least two")
    if a == b:
        return 0.0
    sign = 1
    if a > b:
        a, b, sign = b, a, -1
    span = b - a
    if not math.isfinite(span):
        raise ValueError("interval is too large")
    step = span / steps
    if step == 0:
        raise ValueError("interval is too small for the requested steps")
    total = correction = 0.0
    previous = None
    for index in range(steps + 1):
        x = b if index == steps else a + span * (index / steps)
        if previous is not None and x <= previous:
            raise ValueError("integration points cannot be represented")
        previous = x
        value = _finite(func(x))
        weight = 1 if index == 0 or index == steps else 4 if index % 2 else 2
        factor = weight / 3
        term = (value * step) * factor if step < factor else (value * factor) * step
        adjusted = term - correction
        updated = total + adjusted
        correction = (updated - total) - adjusted
        total = updated
        if not math.isfinite(total) or not math.isfinite(correction):
            raise ValueError("integral exceeds numeric range")
    return sign * total


def derivative(func, x, h=1e-5):
    _function(func)
    x, h = _finite(x), _finite(h)
    if h <= 0:
        raise ValueError("h must be positive")
    left, right = x - h, x + h
    if not math.isfinite(left) or not math.isfinite(right) or left == x or right == x:
        raise ValueError("h cannot be represented at x")
    span = right - left
    if not math.isfinite(span):
        raise ValueError("difference interval is too large")
    right_value, left_value = _finite(func(right)), _finite(func(left))
    difference = right_value - left_value
    if math.isfinite(difference):
        result = difference / span
    else:
        result = (right_value * 0.5 - left_value * 0.5) / (span * 0.5)
    if not math.isfinite(result):
        raise ValueError("derivative exceeds numeric range")
    return result
