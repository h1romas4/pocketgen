// Copyright (c) Microsoft Corporation.
// Licensed under the MIT License.

use alloc::vec::Vec;
use core::ops::Range;
use core::ptr::{self, NonNull};
use core::slice;

use crate::document::{ReadableDocument, WriteableDocument};

const SMALL_ALLOC_CHUNK: usize = 256;
const SMALL_GAP_CHUNK: usize = 16;

/// Most people know how `Vec<T>` works: It has some spare capacity at the end,
/// so that pushing into it doesn't reallocate every single time. A gap buffer
/// is the same thing, but the spare capacity can be anywhere in the buffer.
/// This variant uses the Rust heap instead of virtual memory.
pub struct GapBuffer {
    /// Pointer to the buffer.
    text: NonNull<u8>,
    /// Size of the buffer, including gap.
    commit: usize,
    /// Length of the stored text, NOT including gap.
    text_length: usize,
    /// Gap offset.
    gap_off: usize,
    /// Gap length.
    gap_len: usize,
    /// The current text revision.
    generation: u32,
    /// Increments every time the buffer is modified.
    ///
    /// Distinct edits after restoring a previous revision receive new generations.
    generation_counter: u32,
    buffer: Vec<u8>,
}

impl GapBuffer {
    pub fn new() -> Self {
        Self {
            text: NonNull::dangling(),
            commit: 0,
            text_length: 0,
            gap_off: 0,
            gap_len: 0,
            generation: 0,
            generation_counter: 0,
            buffer: Vec::new(),
        }
    }

    pub fn len(&self) -> usize {
        self.text_length
    }

    pub fn is_empty(&self) -> bool {
        self.text_length == 0
    }

    /// The current revision.
    ///
    /// Automatically updated during all mutations.
    pub fn generation(&self) -> u32 {
        self.generation
    }

    /// Restore a historical revision (for undo/redo).
    pub fn set_generation(&mut self, generation: u32) {
        self.generation = generation;
    }

    /// Assigns a fresh revision, including for document settings that affect serialization.
    pub fn bump_generation(&mut self) {
        self.generation_counter = self.generation_counter.wrapping_add(1);
        self.generation = self.generation_counter;
    }

    /// Move the gap, delete bytes, and return space for the replacement.
    pub fn allocate_gap(&mut self, off: usize, len: usize, delete: usize) -> &mut [u8] {
        // Sanitize parameters
        let off = off.min(self.text_length);
        let delete = delete.min(self.text_length - off);

        // Move the existing gap if it exists
        if off != self.gap_off {
            self.move_gap(off);
        }

        // Delete the text
        if delete > 0 {
            self.delete_text(delete);
        }

        // Enlarge the gap if needed
        if len > self.gap_len {
            self.enlarge_gap(len);
        }

        self.bump_generation();
        unsafe { slice::from_raw_parts_mut(self.text.add(self.gap_off).as_ptr(), self.gap_len) }
    }

    fn move_gap(&mut self, off: usize) {
        if self.gap_len > 0 {
            //
            //                       v gap_off
            // left:  |ABCDEFGHIJKLMN   OPQRSTUVWXYZ|
            //        |ABCDEFGHI   JKLMNOPQRSTUVWXYZ|
            //                  ^ off
            //        move: JKLMN
            //
            //                       v gap_off
            // !left: |ABCDEFGHIJKLMN   OPQRSTUVWXYZ|
            //        |ABCDEFGHIJKLMNOPQRS   TUVWXYZ|
            //                            ^ off
            //        move: OPQRS
            //
            let left = off < self.gap_off;
            let move_src = if left {
                off
            } else {
                self.gap_off + self.gap_len
            };
            let move_dst = if left {
                off + self.gap_len
            } else {
                self.gap_off
            };
            let move_len = if left {
                self.gap_off - off
            } else {
                off - self.gap_off
            };

            unsafe {
                self.text
                    .add(move_src)
                    .copy_to(self.text.add(move_dst), move_len)
            };

            if cfg!(debug_assertions) {
                // Fill the moved-out bytes with 0xCD to make debugging easier.
                unsafe { self.text.add(off).write_bytes(0xCD, self.gap_len) };
            }
        }

        self.gap_off = off;
    }

    fn delete_text(&mut self, delete: usize) {
        if cfg!(debug_assertions) {
            // Fill the deleted bytes with 0xCD to make debugging easier.
            unsafe {
                self.text
                    .add(self.gap_off + self.gap_len)
                    .write_bytes(0xCD, delete)
            };
        }

        self.gap_len += delete;
        self.text_length -= delete;
    }

    fn enlarge_gap(&mut self, len: usize) {
        let (gap_chunk, alloc_chunk) = (SMALL_GAP_CHUNK, SMALL_ALLOC_CHUNK);

        let gap_len_old = self.gap_len;
        let gap_len_new = len
            .checked_add(gap_chunk + gap_chunk - 1)
            .expect("gap size overflow")
            & !(gap_chunk - 1);

        let bytes_old = self.commit;
        let bytes_new = self
            .text_length
            .checked_add(gap_len_new)
            .expect("buffer size overflow");

        if bytes_new > bytes_old {
            let bytes_new = bytes_new
                .checked_add(alloc_chunk - 1)
                .expect("buffer size overflow")
                & !(alloc_chunk - 1);

            self.buffer.resize(bytes_new, 0);
            self.text = unsafe { NonNull::new_unchecked(self.buffer.as_mut_ptr()) };

            self.commit = bytes_new;
        }

        let gap_beg = unsafe { self.text.add(self.gap_off) };
        unsafe {
            ptr::copy(
                gap_beg.add(gap_len_old).as_ptr(),
                gap_beg.add(gap_len_new).as_ptr(),
                self.text_length - self.gap_off,
            )
        };

        if cfg!(debug_assertions) {
            // Fill the moved-out bytes with 0xCD to make debugging easier.
            unsafe {
                gap_beg
                    .add(gap_len_old)
                    .write_bytes(0xCD, gap_len_new - gap_len_old)
            };
        }

        self.gap_len = gap_len_new;
    }

    pub fn commit_gap(&mut self, len: usize) {
        assert!(len <= self.gap_len);
        self.text_length += len;
        self.gap_off += len;
        self.gap_len -= len;
    }

    pub fn replace(&mut self, range: Range<usize>, src: &[u8]) {
        let gap = self.allocate_gap(
            range.start,
            src.len(),
            range.end.saturating_sub(range.start),
        );
        let len = src.len().min(gap.len());
        gap[..len].copy_from_slice(&src[..len]);
        self.commit_gap(len);
    }

    pub fn clear(&mut self) {
        self.gap_off = 0;
        self.gap_len += self.text_length;
        self.bump_generation();
        self.text_length = 0;
    }

    pub fn extract_raw(&self, range: Range<usize>, out: &mut Vec<u8>, mut out_off: usize) {
        let end = range.end.min(self.text_length);
        let mut beg = range.start.min(end);
        out_off = out_off.min(out.len());

        if beg >= end {
            return;
        }

        out.reserve(end - beg);

        while beg < end {
            let chunk = self.read_forward(beg);
            let chunk = &chunk[..chunk.len().min(end - beg)];
            out.splice(out_off..out_off, chunk.iter().copied());
            beg += chunk.len();
            out_off += chunk.len();
        }
    }

    /// Replaces the entire buffer contents with the given `text`.
    /// The method is optimized for the case where the given `text` already matches
    /// the existing contents. Returns `true` if the buffer contents were changed.
    pub fn copy_from(&mut self, src: &dyn ReadableDocument) -> bool {
        let mut off = 0;

        // Find the position at which the contents change.
        loop {
            let dst_chunk = self.read_forward(off);
            let src_chunk = src.read_forward(off);

            let dst_len = dst_chunk.len();
            let src_len = src_chunk.len();
            let len = dst_len.min(src_len);
            let mismatch = dst_chunk[..len] != src_chunk[..len];

            if mismatch {
                break; // The contents differ.
            }
            if len == 0 {
                if dst_len == src_len {
                    return false; // Both done simultaneously. -> Done.
                }
                break; // One of the two is shorter.
            }

            off += len;
        }

        // Update the buffer starting at `off`.
        loop {
            let chunk = src.read_forward(off);
            self.replace(off..usize::MAX, chunk);
            off += chunk.len();

            // No more data to copy -> Done. By checking this _after_ the replace()
            // call, we ensure that the initial `off..usize::MAX` range is deleted.
            // This fixes going from some buffer contents to being empty.
            if chunk.is_empty() {
                return true;
            }
        }
    }

    /// Copies the contents of the buffer into a string.
    pub fn copy_into(&self, dst: &mut dyn WriteableDocument) {
        let mut beg = 0;
        let mut off = 0;

        while {
            let chunk = self.read_forward(off);

            // The first write will be 0..usize::MAX and effectively clear() the destination.
            // Every subsequent write will be usize::MAX..usize::MAX and thus effectively append().
            dst.replace(beg..usize::MAX, chunk);
            beg = usize::MAX;

            off += chunk.len();
            off < self.text_length
        } {}
    }
}

impl Default for GapBuffer {
    fn default() -> Self {
        Self::new()
    }
}

impl ReadableDocument for GapBuffer {
    fn read_forward(&self, off: usize) -> &[u8] {
        let off = off.min(self.text_length);

        let (beg, len) = if off < self.gap_off {
            // Cursor is before the gap: We can read until the start of the gap.
            (off, self.gap_off - off)
        } else {
            // Cursor is after the gap: We can read until the end of the buffer.
            (off + self.gap_len, self.text_length - off)
        };

        unsafe { slice::from_raw_parts(self.text.add(beg).as_ptr(), len) }
    }

    fn read_backward(&self, off: usize) -> &[u8] {
        let off = off.min(self.text_length);

        let (beg, len) = if off <= self.gap_off {
            // Cursor is before the gap: We can read until the beginning of the buffer.
            (0, off)
        } else {
            // Cursor is after the gap: We can read until the end of the gap.
            (self.gap_off + self.gap_len, off - self.gap_off)
        };

        unsafe { slice::from_raw_parts(self.text.add(beg).as_ptr(), len) }
    }
}
