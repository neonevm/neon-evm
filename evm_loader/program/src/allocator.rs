#![allow(clippy::cast_possible_truncation)]

use std::{
    alloc::Layout,
    usize
};

use solana_program::entrypoint::HEAP_START_ADDRESS;


pub struct BumpAllocator;

impl BumpAllocator {
    #[must_use]
    pub fn occupied() -> usize {
        const POSITION_PTR: *const usize = HEAP_START_ADDRESS as *const usize;

        let position = unsafe { core::ptr::read(POSITION_PTR) };
        if position == 0 {
            0_usize
        } else {
            position - (HEAP_START_ADDRESS as usize)
        }
    }
}

unsafe impl std::alloc::GlobalAlloc for BumpAllocator {
    #[inline]
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        const POSITION_PTR: *mut usize = HEAP_START_ADDRESS as *mut usize;

        let mut position = core::ptr::read(POSITION_PTR);
        if position == 0 {
            // First time, set starting position
            position = (HEAP_START_ADDRESS as usize) + core::mem::size_of::<usize>();
        }

        let alignment = layout.align() - 1;

        // round up to multiple of alignment
        position = position.saturating_add(alignment);
        position &= !alignment;

        let top = position.saturating_add(layout.size());
        core::ptr::write(POSITION_PTR, top);

        position as *mut u8
    }

    #[inline]
    unsafe fn dealloc(&self, _: *mut u8, _layout: Layout) {
        // I'm a bump allocator, I don't free
    }
}


#[cfg(target_arch = "bpf")]
#[global_allocator]
static mut A: BumpAllocator = BumpAllocator;