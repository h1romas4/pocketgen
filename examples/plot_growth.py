from pocketgen import plot

# Initial balance 1000, compounded annually.
# Cyan: 3%, yellow: 5%, magenta: 7%; x is years.
def rate_3(years):
    return 1000 * 1.03 ** years


def rate_5(years):
    return 1000 * 1.05 ** years


def rate_7(years):
    return 1000 * 1.07 ** years


plot((rate_3, rate_5, rate_7), 0, 30)
