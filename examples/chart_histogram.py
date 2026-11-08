from pocketgen import graphics_show, hist

# Daily temperatures (C).
temperatures = [18, 19, 20, 20, 21, 21, 21, 22, 22, 23, 24, 25]
counts, edges = hist(temperatures, bins=5)

print("Temperature bins:", edges)
print("Days per bin:", counts)
graphics_show()
