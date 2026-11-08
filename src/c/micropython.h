#ifndef POCKETGEN_MICROPYTHON_H
#define POCKETGEN_MICROPYTHON_H

#include <stddef.h>
#include <stdbool.h>
#include <stdint.h>

typedef void (*pocketgen_output_callback_t)(void *, const uint8_t *, size_t);
/* Return 1 for Ctrl+C, 0 for success, or a negative errno for an error. */
typedef int (*pocketgen_progress_callback_t)(void *, bool);
/* Width/height return a positive size; drawing commands return 0 or -errno. */
typedef int (*pocketgen_graphics_callback_t)(
    void *, uint32_t, int32_t, int32_t, int32_t, int32_t, uint32_t);
/* Return the UTF-8 byte count, 0 when empty, or a negative errno. */
typedef int (*pocketgen_key_callback_t)(void *, uint8_t *, size_t);
enum pocketgen_graphics_command {
    POCKETGEN_GRAPHICS_WIDTH,
    POCKETGEN_GRAPHICS_HEIGHT,
    POCKETGEN_GRAPHICS_CLEAR,
    POCKETGEN_GRAPHICS_PIXEL,
    POCKETGEN_GRAPHICS_LINE,
    POCKETGEN_GRAPHICS_SHOW,
};

/* On nonnegative return, free_bytes receives the GC heap snapshot before VM shutdown. */
int pocketgen_micropython_run(const uint8_t *source, size_t len,
    uint16_t cols, uint16_t rows, pocketgen_output_callback_t callback,
    pocketgen_progress_callback_t progress, pocketgen_graphics_callback_t graphics,
    pocketgen_key_callback_t key,
    void *context, size_t *free_bytes);
const char *pocketgen_micropython_version(void);
int pocketgen_micropython_self_test(void);

#endif
