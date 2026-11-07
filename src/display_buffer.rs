//! Ownership of an RGB565 framebuffer allocated by the C display backend.

use core::ops::{Deref, DerefMut};
use core::ptr::NonNull;

extern "C" {
    fn pocketgen_display_alloc(bytes: usize) -> *mut u16;
    fn pocketgen_display_free(pixels: *mut u16);
}

/// An owned array of RGB565 pixels released by the matching C allocator.
///
/// The slice views borrow the allocation and cannot outlive this owner.
pub(crate) struct DisplayPixels {
    pointer: NonNull<u16>,
    len: usize,
}

impl DisplayPixels {
    /// Allocates and zeroes `len` pixels, returning `None` on overflow or failure.
    pub(crate) fn new(len: usize) -> Option<Self> {
        let bytes = len.checked_mul(core::mem::size_of::<u16>())?;
        let pointer = NonNull::new(unsafe { pocketgen_display_alloc(bytes) })?;
        unsafe { pointer.as_ptr().write_bytes(0, len) };
        Some(Self { pointer, len })
    }
}

impl Deref for DisplayPixels {
    type Target = [u16];

    fn deref(&self) -> &Self::Target {
        unsafe { core::slice::from_raw_parts(self.pointer.as_ptr(), self.len) }
    }
}

impl DerefMut for DisplayPixels {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { core::slice::from_raw_parts_mut(self.pointer.as_ptr(), self.len) }
    }
}

impl Drop for DisplayPixels {
    fn drop(&mut self) {
        unsafe { pocketgen_display_free(self.pointer.as_ptr()) }
    }
}
