use core::slice;
use std::{
    alloc::{GlobalAlloc, Layout},
    mem::{align_of, size_of},
    ptr::NonNull,
};

use linked_list_allocator::Heap;
use solana_program::{entrypoint::HEAP_START_ADDRESS, pubkey::Pubkey};
use static_assertions::{const_assert, const_assert_eq};

//use crate::persistent_state::PersistentState;

// Solana heap constants.
#[allow(clippy::cast_possible_truncation)] // HEAP_START_ADDRESS < usize::max
const SOLANA_HEAP_START_ADDRESS: usize = HEAP_START_ADDRESS as usize;
const SOLANA_HEAP_SIZE: usize = 256 * 1024;

const_assert!(HEAP_START_ADDRESS < (usize::MAX as u64));
const_assert_eq!(SOLANA_HEAP_START_ADDRESS % align_of::<Heap>(), 0);

// Holder account heap constants.
const FIRST_ACCOUNT_DATA_OFFSET: usize =
    /* number of accounts */
    size_of::<u64>() +
    /* duplication marker */ size_of::<u8>() +
    /* is signer? */ size_of::<u8>() +
    /* is writable? */ size_of::<u8>() +
    /* is executable? */ size_of::<u8>() +
    /* original_data_len */ size_of::<u32>() +
    /* key */ size_of::<Pubkey>() +
    /* owner */ size_of::<Pubkey>() +
    /* lamports */ size_of::<u64>() +
    /* factual_data_len */ size_of::<u64>();

#[allow(clippy::cast_possible_truncation)] // HEAP_START_ADDRESS < usize::max
const HOLDER_HEAP_START_ADDRESS: usize = 0x400000000u64 as usize + FIRST_ACCOUNT_DATA_OFFSET + crate::account::STATE_ACCOUNT_HEAP_OFFSET;
const_assert_eq!(HOLDER_HEAP_START_ADDRESS % align_of::<Heap>(), 0);

#[inline]
pub fn acc_allocator() -> AccountAllocator {
    unsafe { HOLDER_ACC_ALLOCATOR }
}

#[inline]
fn solana_default_heap() -> &'static mut Heap {
    // This is legal since all-zero is a valid `Heap`-struct representation
    const HEAP_PTR: *mut Heap = SOLANA_HEAP_START_ADDRESS as *mut Heap;
    let heap = unsafe { &mut *HEAP_PTR };

    if heap.bottom().is_null() {
        let start = (SOLANA_HEAP_START_ADDRESS + size_of::<Heap>()) as *mut u8;
        let size = SOLANA_HEAP_SIZE - size_of::<Heap>();
        unsafe { heap.init(start, size) };
    }

    heap
}

#[inline]
fn holder_account_heap() -> &'static mut Heap {
    // This is legal since all-zero is a valid `Heap`-struct representation
    const HEAP_PTR: *mut Heap = HOLDER_HEAP_START_ADDRESS as *mut Heap;
    let heap = unsafe { &mut *HEAP_PTR };
    // We do not init account heap, it's account's responsibility to initialize it itself.

    heap
}

#[derive(Clone, Copy)]
pub struct AccountAllocator;

impl AccountAllocator {
    fn alloc_impl(&self, layout: Layout) -> Result<NonNull<u8>, ()> {
        holder_account_heap().allocate_first_fit(layout)
    }

    fn dealloc_impl(&self, ptr: *mut u8, layout: Layout) {
        unsafe {
            holder_account_heap().deallocate(NonNull::new_unchecked(ptr), layout);
        }
    }
}

unsafe impl GlobalAlloc for AccountAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        #[allow(clippy::option_if_let_else)]
        if let Ok(non_null) = self.alloc_impl(layout) {
            non_null.as_ptr()
        } else {
            solana_program::log::sol_log("EVM Allocator out of memory");
            std::ptr::null_mut()
        }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        self.dealloc_impl(ptr, layout);
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

unsafe impl allocator_api2::alloc::Allocator for AccountAllocator {
    fn allocate(&self, layout: Layout) -> Result<NonNull<[u8]>, allocator_api2::alloc::AllocError> {
        unsafe {
            self.alloc_impl(layout)
                .map(|ptr| {
                    NonNull::new_unchecked(slice::from_raw_parts_mut(
                        ptr.as_ptr() as *mut u8,
                        layout.size(),
                    ))
                })
                .map_err(|_| allocator_api2::alloc::AllocError)
        }
    }

    unsafe fn deallocate(&self, ptr: NonNull<u8>, layout: Layout) {
        self.dealloc_impl(ptr.as_ptr(), layout);
    }
}

#[derive(Clone, Copy)]
pub struct SolanaAllocator;

impl SolanaAllocator {
    fn alloc_impl(&self, layout: Layout) -> Result<NonNull<u8>, ()> {
        solana_default_heap().allocate_first_fit(layout)
    }

    fn dealloc_impl(&self, ptr: *mut u8, layout: Layout) {
        unsafe {
            solana_default_heap().deallocate(NonNull::new_unchecked(ptr), layout);
        }
    }
}

unsafe impl GlobalAlloc for SolanaAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        #[allow(clippy::option_if_let_else)]
        if let Ok(non_null) = self.alloc_impl(layout) {
            non_null.as_ptr()
        } else {
            solana_program::log::sol_log("EVM Allocator out of memory");
            std::ptr::null_mut()
        }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        self.dealloc_impl(ptr, layout);
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

unsafe impl allocator_api2::alloc::Allocator for SolanaAllocator {
    fn allocate(&self, layout: Layout) -> Result<NonNull<[u8]>, allocator_api2::alloc::AllocError> {
        unsafe {
            self.alloc_impl(layout)
                .map(|ptr| {
                    NonNull::new_unchecked(slice::from_raw_parts_mut(
                        ptr.as_ptr() as *mut u8,
                        layout.size(),
                    ))
                })
                .map_err(|_| allocator_api2::alloc::AllocError)
        }
    }

    unsafe fn deallocate(&self, ptr: NonNull<u8>, layout: Layout) {
        self.dealloc_impl(ptr.as_ptr(), layout);
    }
}


cfg_if::cfg_if! {
    if #[cfg(target_os = "solana")] {
        #[global_allocator]
        static mut DEFAULT: SolanaAllocator = SolanaAllocator;
        pub static mut SOLANA_ALLOCATOR: SolanaAllocator = SolanaAllocator;
        pub static mut HOLDER_ACC_ALLOCATOR: AccountAllocator = AccountAllocator;
    } else {
        use std::alloc::System;

        #[global_allocator]
        static mut DEFAULT: System = System;
        pub static mut EVM: System = System;
        // TODO add newtype pattern around System, implement allocator_api2 trait for it and define HOLDER_ACC_ALLOCATOR.
        pub static mut SOLANA_ALLOCATOR: SolanaAllocator = SolanaAllocator;
        pub static mut HOLDER_ACC_ALLOCATOR: AccountAllocator = AccountAllocator;
    }
}
