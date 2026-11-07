#include <errno.h>
#include <stddef.h>
#include <stdint.h>
#include <string.h>

#include <zephyr/kernel.h>
#include <zephyr/logging/log.h>

#include "py/builtin.h"
#include "py/compile.h"
#include "py/cstack.h"
#include "py/gc.h"
#include "py/lexer.h"
#include "py/runtime.h"
#include "shared/runtime/gchelper.h"
#include "micropython.h"

LOG_MODULE_REGISTER(pocketgen_micropython);

static union {
    uintptr_t alignment;
    uint8_t bytes[128 * 1024];
} heap
#if defined(CONFIG_SOC_ESP32P4)
    __attribute__((section(".ext_ram.bss")))
#endif
    ;
static pocketgen_output_callback_t output_callback;
static pocketgen_progress_callback_t progress_callback;
static void *output_context;
static uint16_t terminal_cols;
static uint16_t terminal_rows;
static unsigned poll_count;
static int64_t last_refresh;

static void pocketgen_check_events(bool force) {
    if (progress_callback == NULL) {
        return;
    }
    int64_t now = k_uptime_get();
    bool refresh = force || now - last_refresh >= 33;
    int result = progress_callback(output_context, refresh);
    if (refresh) {
        last_refresh = k_uptime_get();
    }
    if (result > 0) {
        mp_sched_keyboard_interrupt();
    } else if (result < 0) {
        mp_raise_OSError(-result);
    }
}

void pocketgen_vm_hook(void) {
    if (progress_callback != NULL && ++poll_count >= 1024) {
        poll_count = 0;
        pocketgen_check_events(false);
        k_msleep(1);
    }
}

void mp_hal_stdout_tx_strn_cooked(const char *str, size_t len) {
    if (output_callback != NULL) {
        output_callback(output_context, (const uint8_t *)str, len);
    }
}

void gc_collect(void) {
    gc_collect_start();
    gc_helper_collect_regs_and_stack();
    gc_collect_end();
}

void nlr_jump_fail(void *value) {
    ARG_UNUSED(value);
    LOG_ERR("Uncaught MicroPython exception outside execution boundary");
    k_panic();
    CODE_UNREACHABLE;
}

mp_lexer_t *mp_lexer_new_from_file(qstr filename) {
    ARG_UNUSED(filename);
    mp_raise_OSError(ENOENT);
}

mp_import_stat_t mp_import_stat(const char *path) {
    ARG_UNUSED(path);
    return MP_IMPORT_STAT_NO_EXIST;
}

static mp_obj_t pocketgen_cls(void) {
    mp_print_str(&mp_plat_print, "\x1b[2J\x1b[H");
    return mp_const_none;
}
static MP_DEFINE_CONST_FUN_OBJ_0(pocketgen_cls_obj, pocketgen_cls);

static mp_obj_t pocketgen_size(void) {
    mp_obj_t dimensions[] = {
        mp_obj_new_int_from_uint(terminal_cols),
        mp_obj_new_int_from_uint(terminal_rows),
    };
    return mp_obj_new_tuple(2, dimensions);
}
static MP_DEFINE_CONST_FUN_OBJ_0(pocketgen_size_obj, pocketgen_size);

static mp_obj_t pocketgen_locate(mp_obj_t x_obj, mp_obj_t y_obj) {
    mp_int_t x = mp_obj_get_int(x_obj);
    mp_int_t y = mp_obj_get_int(y_obj);
    if (x < 0 || y < 0 || x >= terminal_cols || y >= terminal_rows) {
        mp_raise_ValueError(MP_ERROR_TEXT("terminal position out of range"));
    }
    mp_printf(&mp_plat_print, "\x1b[%u;%uH", (unsigned)y + 1, (unsigned)x + 1);
    return mp_const_none;
}
static MP_DEFINE_CONST_FUN_OBJ_2(pocketgen_locate_obj, pocketgen_locate);

static mp_obj_t pocketgen_color(size_t n_args, const mp_obj_t *args) {
    mp_int_t fg = mp_obj_get_int(args[0]);
    mp_int_t bg = n_args == 2 ? mp_obj_get_int(args[1]) : 0;
    if (fg < 0 || fg > 15 || bg < 0 || bg > 15) {
        mp_raise_ValueError(MP_ERROR_TEXT("color must be between 0 and 15"));
    }
    unsigned fg_code = (unsigned)(fg < 8 ? 30 + fg : 90 + fg - 8);
    unsigned bg_code = (unsigned)(bg < 8 ? 40 + bg : 100 + bg - 8);
    mp_printf(&mp_plat_print, "\x1b[%u;%um", fg_code, bg_code);
    return mp_const_none;
}
static MP_DEFINE_CONST_FUN_OBJ_VAR_BETWEEN(pocketgen_color_obj, 1, 2, pocketgen_color);

static mp_obj_t pocketgen_refresh(void) {
    pocketgen_check_events(true);
    mp_handle_pending(MP_HANDLE_PENDING_CALLBACKS_AND_EXCEPTIONS);
    return mp_const_none;
}
static MP_DEFINE_CONST_FUN_OBJ_0(pocketgen_refresh_obj, pocketgen_refresh);

static mp_obj_t pocketgen_sleep_ms(mp_obj_t ms_obj) {
    mp_int_t remaining = mp_obj_get_int(ms_obj);
    if (remaining < 0) {
        mp_raise_ValueError(MP_ERROR_TEXT("sleep must be non-negative"));
    }
    pocketgen_check_events(true);
    mp_handle_pending(MP_HANDLE_PENDING_CALLBACKS_AND_EXCEPTIONS);
    while (remaining > 0) {
        int32_t slice = remaining < 10 ? (int32_t)remaining : 10;
        k_msleep(slice);
        remaining -= slice;
        pocketgen_check_events(false);
        mp_handle_pending(MP_HANDLE_PENDING_CALLBACKS_AND_EXCEPTIONS);
    }
    return mp_const_none;
}
static MP_DEFINE_CONST_FUN_OBJ_1(pocketgen_sleep_ms_obj, pocketgen_sleep_ms);

static const mp_rom_map_elem_t pocketgen_module_globals_table[] = {
    { MP_ROM_QSTR(MP_QSTR___name__), MP_ROM_QSTR(MP_QSTR_pocketgen) },
    { MP_ROM_QSTR(MP_QSTR_cls), MP_ROM_PTR(&pocketgen_cls_obj) },
    { MP_ROM_QSTR(MP_QSTR_size), MP_ROM_PTR(&pocketgen_size_obj) },
    { MP_ROM_QSTR(MP_QSTR_locate), MP_ROM_PTR(&pocketgen_locate_obj) },
    { MP_ROM_QSTR(MP_QSTR_color), MP_ROM_PTR(&pocketgen_color_obj) },
    { MP_ROM_QSTR(MP_QSTR_refresh), MP_ROM_PTR(&pocketgen_refresh_obj) },
    { MP_ROM_QSTR(MP_QSTR_sleep_ms), MP_ROM_PTR(&pocketgen_sleep_ms_obj) },
};
static MP_DEFINE_CONST_DICT(pocketgen_module_globals, pocketgen_module_globals_table);

const mp_obj_module_t pocketgen_module = {
    .base = { &mp_type_module },
    .globals = (mp_obj_dict_t *)&pocketgen_module_globals,
};
MP_REGISTER_MODULE(MP_QSTR_pocketgen, pocketgen_module);

int pocketgen_micropython_run(const uint8_t *source, size_t len,
    uint16_t cols, uint16_t rows, pocketgen_output_callback_t callback,
    pocketgen_progress_callback_t progress, void *context, size_t *free_bytes) {
    if (source == NULL || cols == 0 || rows == 0 || callback == NULL ||
        free_bytes == NULL || output_callback != NULL) {
        return -EINVAL;
    }

    output_callback = callback;
    progress_callback = progress;
    output_context = context;
    terminal_cols = cols;
    terminal_rows = rows;
    poll_count = 0;
    last_refresh = k_uptime_get();
    mp_cstack_init_with_sp_here(CONFIG_MAIN_STACK_SIZE / 2);
    gc_init(heap.bytes, heap.bytes + sizeof(heap.bytes));
    mp_init();

    int result = 0;
    nlr_buf_t nlr;
    if (nlr_push(&nlr) == 0) {
        mp_lexer_t *lexer = mp_lexer_new_from_str_len(
            MP_QSTR__lt_stdin_gt_, (const char *)source, len, 0);
        qstr source_name = lexer->source_name;
        mp_parse_tree_t tree = mp_parse(lexer, MP_PARSE_FILE_INPUT);
        mp_obj_t function = mp_compile(&tree, source_name, false);
        mp_call_function_0(function);
        nlr_pop();
    } else {
        mp_obj_print_exception(&mp_plat_print, MP_OBJ_FROM_PTR(nlr.ret_val));
        result = 1;
    }

    gc_info_t info;
    gc_info(&info);
    *free_bytes = info.free;
    mp_deinit();
    output_callback = NULL;
    progress_callback = NULL;
    output_context = NULL;
    return result;
}

const char *pocketgen_micropython_version(void) {
    return MICROPY_VERSION_STRING;
}

int pocketgen_micropython_self_test(void) {
#ifdef CONFIG_POCKETGEN_SELF_TEST
    extern int pocketgen_test_micropython(void);
    int result = pocketgen_test_micropython();
    return result == 0 ? 1 : result;
#else
    return 0;
#endif
}
