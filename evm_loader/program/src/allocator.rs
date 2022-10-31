#![allow(clippy::cast_possible_truncation)]

use std::{
    alloc::Layout,
    usize
};

use solana_program::{entrypoint::HEAP_START_ADDRESS};


pub struct BumpAllocator;

impl BumpAllocator {
    #[allow(dead_code)]
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
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        const POSITION_PTR: *mut usize = HEAP_START_ADDRESS as *mut usize;

        let mut position = core::ptr::read(POSITION_PTR);
        if position == 0 {
            // First time, set starting position
            position = (HEAP_START_ADDRESS as usize) + core::mem::size_of::<usize>();
        }

        let alignment = layout.align() - 1; // layout.align() is power of 2

        // round up to multiple of alignment
        position = position.saturating_add(alignment);
        position &= !alignment;

        let top = position.saturating_add(layout.size());
        core::ptr::write(POSITION_PTR, top);

        position as *mut u8
    }

    unsafe fn dealloc(&self, _: *mut u8, _layout: Layout) {
        // I'm a bump allocator, I don't free
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        // Memory is zeroed by Solana
        self.alloc(layout)

        // #[cfg(target_os = "solana")]
        // solana_program::syscalls::sol_memset_(ptr, 0, size as u64);

        // #[cfg(not(target_os = "solana"))]
        // std::ptr::write_bytes(ptr, 0, size);
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let new_layout = Layout::from_size_align_unchecked(new_size, layout.align());

        let new_ptr = self.alloc(new_layout);

        #[cfg(target_os = "solana")]
        solana_program::syscalls::sol_memcpy_(new_ptr, ptr, std::cmp::min(layout.size(), new_size) as u64);

        #[cfg(not(target_os = "solana"))]
        std::ptr::copy_nonoverlapping(ptr, new_ptr, std::cmp::min(layout.size(), new_size));

        new_ptr
    }
}


#[cfg(target_os = "solana")]
#[global_allocator]
static mut A: BumpAllocator = BumpAllocator;
