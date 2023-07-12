use std::{
    alloc::Layout,
    mem::{align_of, size_of},
    ptr::NonNull,
};

use linked_list_allocator::Heap;
use solana_program::entrypoint::HEAP_START_ADDRESS;
use static_assertions::{const_assert, const_assert_eq};

const HEAP_SIZE: usize = 256 * 1024;

#[allow(clippy::cast_possible_truncation)] // HEAP_START_ADDRESS < usize::max
const EVM_HEAP_START_ADDRESS: usize = HEAP_START_ADDRESS as usize;
const EVM_HEAP_SIZE: usize = HEAP_SIZE;

const_assert!(HEAP_START_ADDRESS < (usize::MAX as u64));
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

cfg_if::cfg_if! {
    if #[cfg(target_os = "solana")] {
        #[global_allocator]
        static mut DEFAULT: SolanaAllocator = SolanaAllocator;
        pub static mut EVM: SolanaAllocator = SolanaAllocator;
    } else {
        use std::alloc::System;

        #[global_allocator]
        static mut DEFAULT: System = System;
        pub static mut EVM: System = System;
    }
}
