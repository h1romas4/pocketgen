#include <errno.h>
#include <stdbool.h>
#include <stdint.h>
#include <string.h>

#include <zephyr/logging/log.h>
#include <zephyr/kernel.h>

#include "py/misc.h"
#include "micropython.h"

LOG_MODULE_REGISTER(pocketgen_tests);

struct captured_output {
    char bytes[2048];
    size_t len;
    bool overflow;
};

static void capture(void *context, const uint8_t *bytes, size_t len) {
    struct captured_output *output = context;
    if (len > sizeof(output->bytes) - 1 - output->len) {
        output->overflow = true;
        return;
    }
    memcpy(output->bytes + output->len, bytes, len);
    output->len += len;
    output->bytes[output->len] = '\0';
}

static int check(const char *source, int expected_status, const char *expected_output) {
    struct captured_output output = {0};
    size_t free_bytes = 0;
    int status = pocketgen_micropython_run(
        (const uint8_t *)source, strlen(source), 80, 24, capture, NULL, NULL, &output, &free_bytes);
    if (status != expected_status || output.overflow ||
        strstr(output.bytes, expected_output) == NULL ||
        free_bytes == 0 || free_bytes > 128 * 1024) {
        LOG_ERR("Python test failed: source=%s, status=%d, overflow=%d, free=%zu, output=%s",
            source, status, output.overflow, free_bytes, output.bytes);
        return -EINVAL;
    }
    return 0;
}

static int check_terminal_controls(void) {
    const char *source =
        "from pocketgen import cls, locate, color\n"
        "cls()\nlocate(0, 0)\nlocate(79, 23)\n"
        "for i in range(16):\n color(i, 15-i)\n"
        "color(7)\n";
    const char *expected =
        "\x1b[2J\x1b[H\x1b[1;1H\x1b[24;80H"
        "\x1b[30;107m\x1b[31;106m\x1b[32;105m\x1b[33;104m"
        "\x1b[34;103m\x1b[35;102m\x1b[36;101m\x1b[37;100m"
        "\x1b[90;47m\x1b[91;46m\x1b[92;45m\x1b[93;44m"
        "\x1b[94;43m\x1b[95;42m\x1b[96;41m\x1b[97;40m\x1b[37;40m";
    struct captured_output output = {0};
    size_t free_bytes = 0;
    int status = pocketgen_micropython_run((const uint8_t *)source, strlen(source),
        80, 24, capture, NULL, NULL, &output, &free_bytes);
    if (status != 0 || output.overflow || strcmp(output.bytes, expected) != 0 || free_bytes == 0) {
        LOG_ERR("Terminal controls failed: status=%d, output=%s", status, output.bytes);
        return -EINVAL;
    }
    return 0;
}

static int check_terminal_size(uint16_t cols, uint16_t rows, const char *expected) {
    const char *source = "from pocketgen import size\nprint(size())\n";
    struct captured_output output = {0};
    size_t free_bytes = 0;
    int status = pocketgen_micropython_run((const uint8_t *)source, strlen(source),
        cols, rows, capture, NULL, NULL, &output, &free_bytes);
    if (status != 0 || output.overflow || strcmp(output.bytes, expected) != 0) {
        LOG_ERR("Terminal size test failed for %u x %u: %s", cols, rows, output.bytes);
        return -EINVAL;
    }
    return 0;
}

struct observed_output {
    struct captured_output output;
    unsigned frames;
    bool first_frame;
    bool second_frame;
};

static int observe_frame(void *context, bool refresh) {
    struct observed_output *observed = context;
    if (refresh) {
        observed->frames++;
        if (strstr(observed->output.bytes, "second") == NULL) {
            observed->first_frame = strstr(observed->output.bytes, "first") != NULL &&
                strstr(observed->output.bytes, "second") == NULL;
        } else {
            observed->second_frame = strstr(observed->output.bytes, "second") != NULL &&
                strstr(observed->output.bytes, "done") == NULL;
        }
    }
    return 0;
}

static int interrupt_run(void *context, bool refresh) {
    ARG_UNUSED(context);
    ARG_UNUSED(refresh);
    return 1;
}

static int fail_progress(void *context, bool refresh) {
    ARG_UNUSED(context);
    ARG_UNUSED(refresh);
    return -EIO;
}

static int check_live_execution(void) {
    const char *source = "from pocketgen import sleep_ms, refresh\n"
        "print('first')\nsleep_ms(40)\nprint('second')\nrefresh()\nprint('done')\n";
    struct observed_output observed = {0};
    size_t free_bytes = 0;
    int64_t start = k_uptime_get();
    int status = pocketgen_micropython_run((const uint8_t *)source, strlen(source),
        80, 24, capture, observe_frame, NULL, &observed, &free_bytes);
    if (status != 0 || !observed.first_frame || !observed.second_frame ||
        strstr(observed.output.bytes, "done") == NULL || k_uptime_get() - start < 40) {
        LOG_ERR("Live frame or sleep test failed");
        return -EINVAL;
    }
    struct captured_output output = {0};
    const char *loop = "while True:\n pass\n";
    status = pocketgen_micropython_run((const uint8_t *)loop, strlen(loop),
        80, 24, capture, interrupt_run, NULL, &output, &free_bytes);
    if (status != 1 || strstr(output.bytes, "KeyboardInterrupt") == NULL) {
        LOG_ERR("Tight-loop interrupt test failed");
        return -EINVAL;
    }
    memset(&output, 0, sizeof(output));
    const char *sleep = "from pocketgen import sleep_ms\nsleep_ms(1000)\nprint('should-not-run')\n";
    status = pocketgen_micropython_run((const uint8_t *)sleep, strlen(sleep),
        80, 24, capture, interrupt_run, NULL, &output, &free_bytes);
    if (status != 1 || strstr(output.bytes, "KeyboardInterrupt") == NULL ||
        strstr(output.bytes, "should-not-run") != NULL) {
        LOG_ERR("Sleep interrupt test failed");
        return -EINVAL;
    }
    memset(&output, 0, sizeof(output));
    const char *refresh = "from pocketgen import refresh\nrefresh()\n";
    status = pocketgen_micropython_run((const uint8_t *)refresh, strlen(refresh),
        80, 24, capture, fail_progress, NULL, &output, &free_bytes);
    if (status != 1 || strstr(output.bytes, "OSError") == NULL) {
        LOG_ERR("Display failure propagation test failed");
        return -EINVAL;
    }
    return 0;
}

int pocketgen_test_micropython(void) {
    struct captured_output output = {0};
    size_t free_bytes = 1;
    if (strcmp(pocketgen_micropython_version(), MICROPY_VERSION_STRING) != 0 ||
        pocketgen_micropython_run(NULL, 0, 80, 24, capture, NULL, NULL, &output, &free_bytes) != -EINVAL ||
        pocketgen_micropython_run((const uint8_t *)"", 0, 0, 24, capture, NULL, NULL, &output, &free_bytes) != -EINVAL ||
        pocketgen_micropython_run((const uint8_t *)"", 0, 80, 0, capture, NULL, NULL, &output, &free_bytes) != -EINVAL ||
        pocketgen_micropython_run((const uint8_t *)"", 0, 80, 24, NULL, NULL, NULL, NULL, &free_bytes) != -EINVAL ||
        pocketgen_micropython_run((const uint8_t *)"", 0, 80, 24, capture, NULL, NULL, &output, NULL) != -EINVAL ||
        free_bytes != 1 ||
        check_live_execution() != 0 ||
        check_terminal_controls() != 0 ||
        check_terminal_size(80, 24, "(80, 24)\n") != 0 ||
        check_terminal_size(1, 1, "(1, 1)\n") != 0 ||
        check_terminal_size(39, 12, "(39, 12)\n") != 0 ||
        check("from pocketgen import locate\nlocate(-1, 0)\n", 1, "ValueError") != 0 ||
        check("from pocketgen import locate\nlocate(80, 0)\n", 1, "ValueError") != 0 ||
        check("from pocketgen import locate\nlocate(0, 24)\n", 1, "ValueError") != 0 ||
        check("from pocketgen import locate\nlocate('x', 0)\n", 1, "TypeError") != 0 ||
        check("from pocketgen import color\ncolor(-1)\n", 1, "ValueError") != 0 ||
        check("from pocketgen import color\ncolor(0, 16)\n", 1, "ValueError") != 0 ||
        check("from pocketgen import color\ncolor(16)\n", 1, "ValueError") != 0 ||
        check("from pocketgen import color\ncolor('red')\n", 1, "TypeError") != 0 ||
        check("from pocketgen import sleep_ms\nsleep_ms(-1)\n", 1, "ValueError") != 0 ||
        check("from pocketgen import sleep_ms\nsleep_ms('x')\n", 1, "TypeError") != 0 ||
        check("print('hello', 2 + 3)\n", 0, "hello 5\n") != 0 ||
        check("assert abs(1.5 + 2.25 - 3.75) < 1e-12\nprint('float-ok')\n", 0, "float-ok") != 0 ||
        check("import math\nassert abs(math.sin(math.pi / 2) - 1.0) < 1e-12\n"
              "print('sin-ok')\n", 0, "sin-ok") != 0 ||
        check("import math\nassert abs(math.sqrt(2.0) ** 2 - 2.0) < 1e-12\n"
              "print('sqrt-ok')\n", 0, "sqrt-ok") != 0 ||
        check("import math\n"
              "assert abs(math.cos(0.0) - 1.0) < 1e-12\n"
              "assert abs(math.cos(math.pi) + 1.0) < 1e-12\n"
              "assert abs(math.atan2(1.0, 1.0) - math.pi / 4) < 1e-12\n"
              "assert abs(math.exp(1.0) - math.e) < 1e-12\n"
              "assert abs(math.log(math.exp(2.0)) - 2.0) < 1e-12\n"
              "assert math.pow(2.0, 3.0) == 8.0\n"
              "assert math.floor(-1.5) == -2 and math.ceil(1.5) == 2\n"
              "assert math.fmod(5.5, 2.0) == 1.5\n"
              "assert math.frexp(8.0) == (0.5, 4)\n"
              "assert math.ldexp(0.5, 4) == 8.0\n"
              "assert math.modf(1.5) == (0.5, 1.0)\n"
              "print('functions-ok')\n", 0, "functions-ok") != 0 ||
        check("import math\nmath.sqrt(-1.0)\n", 1, "ValueError") != 0 ||
        check("raise ValueError('bad input')\n", 1, "ValueError: bad input") != 0 ||
        check("if True print(1)\n", 1, "SyntaxError") != 0 ||
        check("value = 42\nprint(value)\n", 0, "42\n") != 0 ||
        check("try:\n print(value)\nexcept NameError:\n print('reset')\n",
            0, "reset\n") != 0 ||
        check("import gc\nfor i in range(500):\n x = [i] * 100\n"
              "gc.collect()\nprint('gc ok')\n", 0, "gc ok\n") != 0) {
        return -EINVAL;
    }
    const char *baseline = "pass\n";
    const char *allocated = "x = [0] * 2048\n";
    size_t baseline_free = 0;
    size_t allocated_free = 0;
    int baseline_status = pocketgen_micropython_run((const uint8_t *)baseline, strlen(baseline),
        80, 24, capture, NULL, NULL, &output, &baseline_free);
    int allocated_status = pocketgen_micropython_run((const uint8_t *)allocated, strlen(allocated),
        80, 24, capture, NULL, NULL, &output, &allocated_free);
    if (baseline_status != 0 || allocated_status != 0 ||
        baseline_free <= allocated_free || baseline_free - allocated_free < 7000) {
        LOG_ERR("GC allocation test: status=%d/%d free=%zu/%zu output=%s",
            baseline_status, allocated_status, baseline_free, allocated_free, output.bytes);
        return -EINVAL;
    }
    LOG_INF("MicroPython integration tests passed");
    return 0;
}
