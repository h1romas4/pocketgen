#include <errno.h>
#include <stdint.h>

#include <zephyr/device.h>
#include <zephyr/devicetree.h>
#include <zephyr/drivers/uart.h>
#include <zephyr/kernel.h>
#if defined(CONFIG_ARCH_POSIX)
#include <native_rtc.h>
#endif

static const struct device *const input = DEVICE_DT_GET(DT_CHOSEN(zephyr_console));

int pocketgen_input_init(void) {
    return device_is_ready(input) ? 0 : -ENODEV;
}

int pocketgen_input_poll(uint8_t *byte) {
    int result = uart_poll_in(input, byte);
    return result == -1 ? 0 : (result == 0 ? 1 : result);
}

void pocketgen_input_wait(void) {
    k_msleep(10);
}

int pocketgen_syntax_highlight_enabled(void) {
    return IS_ENABLED(CONFIG_POCKETGEN_SYNTAX_HIGHLIGHT);
}

int pocketgen_highlight_benchmark_enabled(void) {
    return IS_ENABLED(CONFIG_POCKETGEN_HIGHLIGHT_BENCHMARK);
}

int pocketgen_autostart_cube_enabled(void) {
    return IS_ENABLED(CONFIG_POCKETGEN_AUTOSTART_CUBE);
}

uint32_t pocketgen_benchmark_ticks(void) {
#if defined(CONFIG_ARCH_POSIX)
    return (uint32_t)native_rtc_gettime_us(RTC_CLOCK_PSEUDOHOSTREALTIME);
#else
    return k_cycle_get_32();
#endif
}

uint64_t pocketgen_benchmark_ticks_to_us(uint64_t ticks) {
#if defined(CONFIG_ARCH_POSIX)
    return ticks;
#else
    return k_cyc_to_us_floor64(ticks);
#endif
}
