import math
from pocketgen import plot

# Height (m) versus time (s), launched at 20 m/s.
# Cyan: 45 degrees, yellow: 60 degrees.
def height_45(t):
    return 20 * math.sin(math.pi / 4) * t - 4.905 * t * t


def height_60(t):
    return 20 * math.sin(math.pi / 3) * t - 4.905 * t * t


plot((height_45, height_60), 0, 4, ymin=0, ymax=20)
