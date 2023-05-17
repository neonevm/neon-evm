use std::{alloc::{Layout, GlobalAlloc}, ops::Deref, ptr::NonNull};

const BUFFER_ALIGN: usize = 1;

pub struct Buffer {
    ptr: NonNull<u8>,
    len: usize
}

impl Buffer {
    #[must_use]
    pub fn new(v: &[u8]) -> Self {
        if v.is_empty() {
            return Self::empty();
        }

        unsafe {
            let len = v.len();

            let layout = Layout::from_size_align_unchecked(len, BUFFER_ALIGN);
            let ptr = crate::allocator::EVM.alloc(layout);
            if ptr.is_null() {
                std::alloc::handle_alloc_error(layout);
            }

            cfg_if::cfg_if! {
                if #[cfg(target_os = "solana")] {
                    solana_program::syscalls::sol_memcpy_(ptr, v.as_ptr(), len as u64);
                } else {
                    std::ptr::copy_nonoverlapping(v.as_ptr(), ptr, len);
                }
            }

            Self { ptr: NonNull::new_unchecked(ptr), len }
        }
    }

    #[must_use]
    pub fn empty() -> Self {
        Self {
            ptr: NonNull::dangling(),
            len: 0
        }
    }

    #[inline]
    #[must_use]
    pub fn get_or_default(&self, index: usize) -> u8 {
        if index < self.len {
            unsafe { self.ptr.as_ptr().add(index).read() }
        } else {
            0
        }
    }
}

impl Drop for Buffer {
    fn drop(&mut self) {
        if self.len == 0 {
            return;
        }

        unsafe {
            let layout = Layout::from_size_align_unchecked(self.len, BUFFER_ALIGN);
            crate::allocator::EVM.dealloc(self.ptr.as_ptr(), layout);
        }
    }
}

impl Deref for Buffer {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        unsafe {
            std::slice::from_raw_parts(self.ptr.as_ptr(), self.len)
        }
    }
}

impl Clone for Buffer {
    fn clone(&self) -> Self {
        Self::new(self)
    }
}

impl Default for Buffer {
    fn default() -> Self {
        Self::empty()
    }
}

impl serde::Serialize for Buffer {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer 
    {
        serializer.serialize_bytes(self)
    }
}

impl<'de> serde::Deserialize<'de> for Buffer {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de> 
    {
        struct BytesVisitor;

        impl<'de> serde::de::Visitor<'de> for BytesVisitor
        {
            type Value = Buffer;
        
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("EVM Buffer")
            }
        
            fn visit_bytes<E>(self, v: &[u8]) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                Ok(Buffer::new(v))
            }
        }

        deserializer.deserialize_bytes(BytesVisitor)
    }
}