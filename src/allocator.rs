use core::alloc::{GlobalAlloc, Layout};

struct PsramAllocator;

extern "C" {
    fn pocketgen_rust_alloc(size: usize, align: usize) -> *mut u8;
    fn pocketgen_rust_free(ptr: *mut u8);
}

unsafe impl GlobalAlloc for PsramAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        pocketgen_rust_alloc(layout.size(), layout.align())
    }

    unsafe fn dealloc(&self, ptr: *mut u8, _layout: Layout) {
        pocketgen_rust_free(ptr);
    }
}

#[global_allocator]
static ALLOCATOR: PsramAllocator = PsramAllocator;

pub(crate) fn self_test() -> bool {
    for (size, align) in [(1, 1), (32, 8), (257, 64), (4097, 4096)] {
        let layout = Layout::from_size_align(size, align).unwrap();
        let ptr = unsafe { ALLOCATOR.alloc(layout) };
        if ptr.is_null() {
            return false;
        }
        let aligned = (ptr as usize) % align == 0;
        unsafe {
            ptr.write_bytes(0xa5, size);
        }
        let intact = unsafe { core::slice::from_raw_parts(ptr, size) }
            .iter()
            .all(|byte| *byte == 0xa5);
        unsafe {
            ALLOCATOR.dealloc(ptr, layout);
        }
        if !aligned || !intact {
            return false;
        }
    }
    true
}
