use std::{
    alloc::Layout,
    mem::{align_of, size_of},
    ptr::NonNull,
};

use linked_list_allocator::Heap;
use solana_program::entrypoint::HEAP_START_ADDRESS;
use static_assertions::const_assert_eq;

const HEAP_SIZE: usize = 256 * 1024;

#[allow(clippy::cast_possible_truncation)] // HEAP_START_ADDRESS < usize::max
const BUMP_HEAP_START_ADDRESS: usize = HEAP_START_ADDRESS as usize;
const BUMP_HEAP_SIZE: usize = 100 * 1024;
const BUMP_HEAP_END_ADDRESS: usize = BUMP_HEAP_START_ADDRESS + BUMP_HEAP_SIZE;

const EVM_HEAP_START_ADDRESS: usize = BUMP_HEAP_END_ADDRESS;
const EVM_HEAP_SIZE: usize = HEAP_SIZE - BUMP_HEAP_SIZE;

const_assert_eq!(EVM_HEAP_START_ADDRESS % align_of::<Heap>(), 0);

#[inline]
unsafe fn heap() -> &'static mut Heap {
    // This is legal since all-zero is a valid `Heap`-struct representation
    const HEAP_PTR: *mut Heap = EVM_HEAP_START_ADDRESS as *mut Heap;
    let heap = &mut *HEAP_PTR;

    if heap.bottom().is_null() {
        let start = (EVM_HEAP_START_ADDRESS + size_of::<Heap>()) as *mut u8;
        let size = EVM_HEAP_SIZE - size_of::<Heap>();
        heap.init(start, size);
    }

    heap
}

pub struct SolanaAllocator;

unsafe impl std::alloc::GlobalAlloc for SolanaAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if let Ok(non_null) = heap().allocate_first_fit(layout) {
            non_null.as_ptr()
        } else {
            solana_program::log::sol_log("EVM Allocator out of memory");
            std::ptr::null_mut()
        }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        heap().deallocate(NonNull::new_unchecked(ptr), layout);
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        let ptr = self.alloc(layout);

        if !ptr.is_null() {
            #[cfg(target_os = "solana")]
            solana_program::syscalls::sol_memset_(ptr, 0, layout.size() as u64);
            #[cfg(not(target_os = "solana"))]
            std::ptr::write_bytes(ptr, 0, layout.size());
        }

        ptr
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let new_layout = Layout::from_size_align_unchecked(new_size, layout.align());
        let new_ptr = self.alloc(new_layout);

        if !new_ptr.is_null() {
            let copy_bytes = std::cmp::min(layout.size(), new_size);

            #[cfg(target_os = "solana")]
            solana_program::syscalls::sol_memcpy_(new_ptr, ptr, copy_bytes as u64);
            #[cfg(not(target_os = "solana"))]
            std::ptr::copy_nonoverlapping(ptr, new_ptr, copy_bytes);

            self.dealloc(ptr, layout);
        }

        new_ptr
    }
}

struct BumpAllocator;

impl BumpAllocator {
    const POSITION_PTR: *mut usize = BUMP_HEAP_START_ADDRESS as *mut usize;

    #[allow(dead_code)]
    pub fn occupied() -> usize {
        let position = unsafe { core::ptr::read(Self::POSITION_PTR) };
        if position == 0 {
            0_usize
        } else {
            position - BUMP_HEAP_START_ADDRESS
        }
    }
}

unsafe impl std::alloc::GlobalAlloc for BumpAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let mut position = core::ptr::read(Self::POSITION_PTR);
        if position == 0 {
            // First time, set starting position
            position = BUMP_HEAP_START_ADDRESS + core::mem::size_of::<usize>();
        }

        let alignment = layout.align() - 1; // layout.align() is power of 2

        // round up to multiple of alignment
        position = (position + alignment) & !alignment;

        let top = position.saturating_add(layout.size());
        if top < BUMP_HEAP_END_ADDRESS {
            core::ptr::write(Self::POSITION_PTR, top);
            position as *mut u8
        } else {
            solana_program::log::sol_log("Bump Allocator out of memory");
            std::ptr::null_mut()
        }
    }

    unsafe fn dealloc(&self, _: *mut u8, _layout: Layout) {
        // I'm a bump allocator, I don't free
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        self.alloc(layout)

        // Memory is zeroed by Solana
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let new_layout = Layout::from_size_align_unchecked(new_size, layout.align());
        let new_ptr = self.alloc(new_layout);

        if !new_ptr.is_null() {
            let copy_bytes = std::cmp::min(layout.size(), new_size);

            #[cfg(target_os = "solana")]
            solana_program::syscalls::sol_memcpy_(new_ptr, ptr, copy_bytes as u64);
            #[cfg(not(target_os = "solana"))]
            std::ptr::copy_nonoverlapping(ptr, new_ptr, copy_bytes);
        }

        new_ptr
    }
}

cfg_if::cfg_if! {
    if #[cfg(target_os = "solana")] {
        #[global_allocator]
        static mut DEFAULT: BumpAllocator = BumpAllocator;
        pub static mut EVM: SolanaAllocator = SolanaAllocator;
    } else {
        use std::alloc::System;

        #[global_allocator]
        static mut DEFAULT: System = System;
        pub static mut EVM: System = System;
    }
}
