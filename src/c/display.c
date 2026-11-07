#include <errno.h>
#include <stddef.h>
#include <stdint.h>
#include <stdlib.h>

#include <zephyr/device.h>
#include <zephyr/devicetree.h>
#include <zephyr/drivers/display.h>
#if defined(CONFIG_SOC_ESP32P4)
#include <zephyr/multi_heap/shared_multi_heap.h>
#endif

static const struct device *const display = DEVICE_DT_GET(DT_CHOSEN(zephyr_display));

void *pocketgen_display_alloc(size_t bytes) {
#if defined(CONFIG_SOC_ESP32P4)
    uint8_t *allocation = shared_multi_heap_aligned_alloc(
        SMH_REG_ATTR_EXTERNAL, CONFIG_ESP32_CACHE_L2_LINE_SIZE, bytes + 2);
    return allocation == NULL ? NULL : allocation + 2;
#else
    return malloc(bytes);
#endif
}

void pocketgen_display_free(void *pixels) {
#if defined(CONFIG_SOC_ESP32P4)
    if (pixels != NULL) {
        shared_multi_heap_free((uint8_t *)pixels - 2);
    }
#else
    free(pixels);
#endif
}

int pocketgen_display_init(uint16_t *width, uint16_t *height) {
    if (!device_is_ready(display)) {
        return -ENODEV;
    }

    struct display_capabilities caps;
    display_get_capabilities(display, &caps);
    if ((caps.supported_pixel_formats & PIXEL_FORMAT_RGB_565) == 0) {
        return -ENOTSUP;
    }
    int result = display_set_pixel_format(display, PIXEL_FORMAT_RGB_565);
    if (result != 0) {
        return result;
    }
    *width = caps.x_resolution;
    *height = caps.y_resolution;
    return display_blanking_off(display);
}

int pocketgen_display_present(const uint16_t *pixels, uint16_t width, uint16_t height) {
    const struct display_buffer_descriptor descriptor = {
        .buf_size = (uint32_t)width * height * sizeof(*pixels),
        .width = width,
        .height = height,
        .pitch = width,
    };
    return display_write(display, 0, 0, &descriptor, pixels);
}
