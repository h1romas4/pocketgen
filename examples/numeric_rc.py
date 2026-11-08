import math
from pocketgen import (
    derivative, graphics_line, graphics_show, integrate, plot, plot_position,
    refresh, solve,
)

# Capacitor discharge with a one-second RC time constant.
def voltage(t):
    return math.exp(-t)


half_time = solve(lambda t: voltage(t) - 0.5, 0, 3)
area = integrate(voltage, 0, 3)
slope = derivative(voltage, 1)
v1 = voltage(1)

# Cyan: voltage. Magenta: half voltage. Yellow: tangent at 1 s.
def tangent(t):
    return v1 + slope * (t - 1)


bounds = plot((voltage, tangent, lambda t: 0.5), 0, 3, ymin=0, ymax=1,
              colors=(0x07ff, 0xffe0, 0xf81f))

# Dark hatching shows the integral; green and white crosses mark the two points.
left, baseline = plot_position(0, 0, bounds)
right, _ = plot_position(3, 0, bounds)
for pixel_x in range(left, right + 1, 12):
    t = 3 * (pixel_x - left) / (right - left)
    x, y = plot_position(t, voltage(t), bounds)
    if y + 2 < baseline:
        graphics_line(x, y + 2, x, baseline - 1, 0x2108)

for t, value, color in ((half_time, 0.5, 0x07e0), (1, v1, 0xffff)):
    x, y = plot_position(t, value, bounds)
    graphics_line(x - 3, y, x + 3, y, color)
    graphics_line(x, y - 3, x, y + 3, color)
refresh()

print("RC discharge (time constant: 1 s)")
print("Half-voltage time:", half_time, "s")
print("Voltage integral 0-3 s:", area, "V s")
print("Slope at 1 s:", slope, "V/s")
print("Cyan: voltage; magenta: half voltage")
print("Green: half-time; yellow: tangent")
print("White: tangent point; shaded: integral")
graphics_show()
