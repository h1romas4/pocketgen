#include <stddef.h>
#include <stdint.h>
#include <zephyr/multi_heap/shared_multi_heap.h>

void *pocketgen_rust_alloc(size_t size, size_t align) {
    return shared_multi_heap_aligned_alloc(SMH_REG_ATTR_EXTERNAL, align, size);
}

void pocketgen_rust_free(void *ptr) {
    shared_multi_heap_free(ptr);
}
