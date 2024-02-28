use std::alloc::{GlobalAlloc, Layout};
use std::ops::Range;

use solana_program::program_memory::{sol_memcpy, sol_memset};

use crate::error::Error;

use super::utils::checked_next_multiple_of_32;
use super::Buffer;

const MAX_MEMORY_SIZE: usize = 64 * 1024;
const MEMORY_CAPACITY: usize = 1024;
const MEMORY_ALIGN: usize = 1;

static_assertions::const_assert!(MEMORY_ALIGN.is_power_of_two());

pub struct Memory {
    data: *mut u8,
    capacity: usize,
    size: usize,
}

impl Memory {
    pub fn new() -> Self {
        Self::with_capacity(MEMORY_CAPACITY)
    }

    pub fn with_capacity(capacity: usize) -> Self {
        unsafe {
            let layout = Layout::from_size_align_unchecked(capacity, MEMORY_ALIGN);
            let data = crate::allocator::HOLDER_ACC_ALLOCATOR.alloc_zeroed(layout);
            if data.is_null() {
                std::alloc::handle_alloc_error(layout);
            }

            Self {
                data,
                capacity,
                size: 0,
            }
        }
    }

    pub fn from_buffer(v: &[u8]) -> Self {
        let capacity = v.len().next_power_of_two().max(MEMORY_CAPACITY);

        unsafe {
            let layout = Layout::from_size_align_unchecked(capacity, MEMORY_ALIGN);
            let data = crate::allocator::HOLDER_ACC_ALLOCATOR.alloc_zeroed(layout);
            if data.is_null() {
                std::alloc::handle_alloc_error(layout);
            }

            std::ptr::copy_nonoverlapping(v.as_ptr(), data, v.len());

            Self {
                data,
                capacity,
                size: v.len(),
            }
        }
    }

    #[cfg(not(target_os = "solana"))]
    pub fn to_vec(&self) -> Vec<u8> {
        let slice = unsafe { std::slice::from_raw_parts(self.data, self.size) };
        slice.to_vec()
    }

    #[inline]
    fn realloc(&mut self, offset: usize, length: usize) -> Result<(), Error> {
        let required_size = offset
            .checked_add(length)
            .ok_or(Error::MemoryAccessOutOfLimits(offset, length))?;

        let new_size = checked_next_multiple_of_32(required_size)
            .ok_or(Error::MemoryAccessOutOfLimits(offset, length))?;

        if new_size > self.size {
            self.size = new_size;
        }

        if new_size <= self.capacity {
            return Ok(());
        }

        let new_capacity = new_size
            .checked_next_power_of_two()
            .ok_or(Error::MemoryAccessOutOfLimits(offset, length))?;
        if new_capacity > MAX_MEMORY_SIZE {
            return Err(Error::MemoryAccessOutOfLimits(offset, length));
        }

        unsafe {
            let old_layout = Layout::from_size_align_unchecked(self.capacity, MEMORY_ALIGN);
            let new_data =
                crate::allocator::HOLDER_ACC_ALLOCATOR.realloc(self.data, old_layout, new_capacity);
            if new_data.is_null() {
                let layout = Layout::from_size_align_unchecked(new_capacity, MEMORY_ALIGN);
                std::alloc::handle_alloc_error(layout);
            }

            let slice = core::slice::from_raw_parts_mut(new_data, new_capacity);
            sol_memset(&mut slice[self.capacity..], 0, new_capacity - self.capacity);

            self.data = new_data;
            self.capacity = new_capacity;
        }

        Ok(())
    }

    #[inline]
    #[must_use]
    pub fn size(&self) -> usize {
        self.size
    }

    pub fn read(&mut self, offset: usize, length: usize) -> Result<&[u8], Error> {
        if length == 0_usize {
            return Ok(&[]);
        }

        self.realloc(offset, length)?;

        let slice = unsafe {
            let data = self.data.add(offset);
            core::slice::from_raw_parts(data, length)
        };

        Ok(slice)
    }

    pub fn read_32(&mut self, offset: usize) -> Result<&[u8; 32], Error> {
        self.realloc(offset, 32)?;

        let array: &[u8; 32] = unsafe {
            let data = self.data.add(offset);
            &*(data as *const [u8; 32])
        };

        Ok(array)
    }

    pub fn write_32(&mut self, offset: usize, value: &[u8; 32]) -> Result<(), Error> {
        self.realloc(offset, 32)?;

        unsafe {
            let data = self.data.add(offset);
            core::ptr::copy_nonoverlapping(value.as_ptr(), data, 32);
        };

        Ok(())
    }

    pub fn write_byte(&mut self, offset: usize, value: u8) -> Result<(), Error> {
        self.realloc(offset, 1)?;

        unsafe {
            let data = self.data.add(offset);
            *data = value;
        };

        Ok(())
    }

    pub fn write_buffer(
        &mut self,
        offset: usize,
        length: usize,
        source: &[u8],
        source_offset: usize,
    ) -> Result<(), Error> {
        if length == 0_usize {
            return Ok(());
        }

        self.realloc(offset, length)?;

        let data = unsafe {
            let data = self.data.add(offset);
            core::slice::from_raw_parts_mut(data, length)
        };

        match source_offset {
            source_offset if source_offset >= source.len() => {
                sol_memset(data, 0, length);
            }
            source_offset if (source_offset + length) > source.len() => {
                let source = &source[source_offset..];

                data[..source.len()].copy_from_slice(source);
                data[source.len()..].fill(0_u8);
            }
            source_offset => {
                let source = &source[source_offset..source_offset + length];
                sol_memcpy(data, source, length);
            }
        }

        Ok(())
    }

    #[inline]
    pub fn write_range(&mut self, range: &Range<usize>, source: &[u8]) -> Result<(), Error> {
        self.write_buffer(range.start, range.len(), source, 0)
    }

    #[inline]
    pub fn read_buffer(&mut self, offset: usize, length: usize) -> Result<Buffer, Error> {
        let slice = self.read(offset, length)?;
        Ok(Buffer::from_slice(slice))
    }
}

impl Drop for Memory {
    fn drop(&mut self) {
        unsafe {
            let layout = Layout::from_size_align_unchecked(self.capacity, MEMORY_ALIGN);
            crate::allocator::SOLANA_ALLOCATOR.dealloc(self.data, layout);
        }
    }
}

impl serde::Serialize for Memory {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let data = unsafe { std::slice::from_raw_parts(self.data, self.capacity) };
        serializer.serialize_bytes(&data[..self.size()])
    }
}

impl<'de> serde::Deserialize<'de> for Memory {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct BytesVisitor;

        impl<'de> serde::de::Visitor<'de> for BytesVisitor {
            type Value = Memory;

            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("EVM Memory")
            }

            fn visit_bytes<E>(self, v: &[u8]) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                if v.len() % 32 != 0 {
                    return Err(E::invalid_length(v.len(), &self));
                }

                Ok(Memory::from_buffer(v))
            }
        }

        deserializer.deserialize_bytes(BytesVisitor)
    }
}
