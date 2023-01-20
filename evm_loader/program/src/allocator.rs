#![cfg(target_os = "solana")]

use std::{
    alloc::Layout,
    ptr::NonNull, mem::{size_of, align_of}
};

use solana_program::{entrypoint::HEAP_START_ADDRESS};
use static_assertions::const_assert_eq;
use linked_list_allocator::Heap;

const HEAP_SIZE: usize = 256 * 1024;

#[inline]
unsafe fn heap() -> &'static mut Heap {
    const_assert_eq!(HEAP_START_ADDRESS % (align_of::<Heap>() as u64), 0);

    const HEAP_PTR: *mut Heap = HEAP_START_ADDRESS as *mut Heap;
    let heap = &mut *HEAP_PTR;

    if heap.bottom().is_null() {
        let start = (HEAP_START_ADDRESS + size_of::<Heap>() as u64) as *mut u8;
        let size = HEAP_SIZE - size_of::<Heap>();
        heap.init(start, size);
    }

    heap
}

struct SolanaAllocator;

unsafe impl std::alloc::GlobalAlloc for SolanaAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        heap().allocate_first_fit(layout)
            .map_or(core::ptr::null_mut(), NonNull::as_ptr)
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        heap().deallocate(NonNull::new_unchecked(ptr), layout);
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        let ptr = self.alloc(layout);

        solana_program::syscalls::sol_memset_(ptr, 0, layout.size() as u64);

        ptr
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let new_layout = Layout::from_size_align_unchecked(new_size, layout.align());
        let new_ptr = self.alloc(new_layout);

        solana_program::syscalls::sol_memcpy_(new_ptr, ptr, std::cmp::min(layout.size(), new_size) as u64);

        self.dealloc(ptr, layout);

        new_ptr
    }
}


#[cfg(target_os = "solana")]
#[global_allocator]
static mut A: SolanaAllocator = SolanaAllocator;
