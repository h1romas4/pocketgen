from pocketgen import graphics_show, linear_fit, scatter

# Measured distance (m) at each time (s).
time = [0, 1, 2, 3, 4, 5]
distance = [0.2, 1.8, 4.1, 6.2, 7.9, 10.1]
slope, intercept, r2 = linear_fit(time, distance)

scatter(time, distance, fit=lambda t: slope * t + intercept)
print("distance = speed * time + offset")
print("speed:", slope, "m/s")
print("offset:", intercept, "m")
print("R2:", r2)
graphics_show()
