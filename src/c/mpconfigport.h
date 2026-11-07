#include <alloca.h>
#include <stdint.h>

typedef long mp_off_t;

#define MP_STATE_PORT MP_STATE_VM
#define MICROPY_MPHALPORT_H "mphalport.h"
#define MICROPY_CONFIG_ROM_LEVEL (MICROPY_CONFIG_ROM_LEVEL_MINIMUM)
#define MICROPY_ENABLE_COMPILER (1)
#define MICROPY_ENABLE_GC (1)
#define MICROPY_PY_GC (1)
#define MICROPY_PY_SYS (1)
#define MICROPY_PY_SYS_PLATFORM "zephyr"
#define MICROPY_KBD_EXCEPTION (1)
#define MICROPY_VM_HOOK_LOOP do { pocketgen_vm_hook(); } while (0);
#define MICROPY_STACK_CHECK (1)
#define MICROPY_STACK_CHECK_MARGIN (1024)
#define MICROPY_ERROR_REPORTING (MICROPY_ERROR_REPORTING_DETAILED)

void pocketgen_vm_hook(void);
