use std::{
    alloc::{Layout, alloc_zeroed, realloc, dealloc},
    cell::Cell,
};
use solana_program::program_memory::{sol_memset, sol_memcpy};

use crate::error::Error;
use super::tracing_event;

const MAX_MEMORY_SIZE: usize = 64 * 1024;
const MEMORY_ALIGN: usize = 32;

pub struct Memory {
    data: *mut u8,
    capacity: usize,
    size: Cell<usize>,
}

impl Memory {
    pub fn new() -> Self {
        const DEFAULT_CAPACITY: usize = 1024; 

        Self::with_capacity(DEFAULT_CAPACITY)
    }

    pub fn with_capacity(capacity: usize) -> Self {
        unsafe {
            let layout = Layout::from_size_align_unchecked(capacity, MEMORY_ALIGN);
            let data = alloc_zeroed(layout);

            Self { data, capacity, size: Cell::new(0) }
        }
    }

    #[allow(dead_code)]
    pub fn to_vec(&self) -> Vec<u8> {
        let slice = unsafe {
            let len = self.size.get();
            std::slice::from_raw_parts(self.data, len)
        };
        slice.to_vec()
    }

    #[inline]
    fn realloc(&mut self, offset: usize, length: usize) -> Result<(), Error> {
        let new_size = offset.saturating_add(length);

        if new_size <= self.capacity {
            return Ok(());
        }

        let size = new_size.next_power_of_two();
        if size > MAX_MEMORY_SIZE {
            return Err(Error::MemoryAccessOutOfLimits(offset, length));
        }

        self.data = unsafe {
            let old_layout = Layout::from_size_align_unchecked(self.capacity, MEMORY_ALIGN);
            realloc(self.data, old_layout, size)
        };

        let slice = unsafe { core::slice::from_raw_parts_mut(self.data, size) };
        sol_memset(&mut slice[self.capacity..], 0, size - self.capacity);
        
        self.capacity = size;

        Ok(())
    }

    #[inline]
    fn extend_size(&self, new_size: usize) {
        let new_size = (new_size + 31_usize) & !31_usize; // next multiple of 32
        if new_size > Cell::get(&self.size) {
            Cell::set(&self.size, new_size);
        }
    }

    pub fn size(&self) -> usize {
        Cell::get(&self.size)
    }

    pub fn read(&self, offset: usize, length: usize) -> Result<&[u8], Error> {
        if length == 0_usize {
            return Ok(&[])
        }

        if offset.saturating_add(length) > self.capacity {
            return Err(Error::MemoryAccessOutOfLimits(offset, length));
        }

        let slice = unsafe {
            let data = self.data.add(offset);
            core::slice::from_raw_parts(data, length) 
        };

        self.extend_size(offset + length);

        Ok(slice)
    }

    pub fn read_32(&self, offset: usize) -> Result<&[u8; 32], Error> {
        if offset.saturating_add(32) > self.capacity {
            return Err(Error::MemoryAccessOutOfLimits(offset, 32));
        }

        let array: &[u8; 32] = unsafe {
            let data = self.data.add(offset);
            &*(data as *const [u8; 32])
        };

        self.extend_size(offset + 32);

        Ok(array)
    }

    pub fn write_32(&mut self, offset: usize, value: &[u8; 32]) -> Result<(), Error> {
        tracing_event!(super::tracing::Event::MemorySet { 
            offset, data: value.to_vec()
        });

        self.realloc(offset, 32)?;

        unsafe {
            let data = self.data.add(offset);
            core::ptr::copy_nonoverlapping(value.as_ptr(), data, 32);
        };

        self.extend_size(offset + 32);

        Ok(())
    }

    pub fn write_byte(&mut self, offset: usize, value: u8) -> Result<(), Error> {
        tracing_event!(super::tracing::Event::MemorySet { 
            offset, data: vec![value]
        });

        self.realloc(offset, 1)?;

        unsafe {
            let data = self.data.add(offset);
            *data = value;
        };

        self.extend_size(offset + 1);

        Ok(())
    }

    pub fn write_buffer(&mut self, offset: usize, length: usize, source: &[u8], source_offset: usize) -> Result<(), Error> {
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
                tracing_event!(super::tracing::Event::MemorySet {
                    offset, data: vec![0; length]
                });

                sol_memset(data, 0, length);
            },
            source_offset if (source_offset + length) > source.len() => {
                let source = &source[source_offset..];

                tracing_event!(super::tracing::Event::MemorySet {
                    offset,
                    data: {
                        let mut buffer = vec![0_u8; length];
                        buffer[..source.len()].copy_from_slice(source);
                        buffer
                    }
                });

                data[..source.len()].copy_from_slice(source);
                data[source.len()..].fill(0_u8);
            },
            source_offset => {
                let source = &source[source_offset..source_offset+length];

                tracing_event!(super::tracing::Event::MemorySet {
                    offset, data: source.to_vec()
                });

                sol_memcpy(data, source, length);
            }
        }

        self.extend_size(offset + length);

        Ok(())
    }
}

impl Drop for Memory {
    fn drop(&mut self) {
        unsafe {
            let layout = Layout::from_size_align_unchecked(self.capacity, MEMORY_ALIGN);
            dealloc(self.data, layout);
        }
    }
}



impl serde::Serialize for Memory {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer 
    {
        let data = unsafe {
            std::slice::from_raw_parts(self.data, self.capacity)
        };
        serializer.serialize_bytes(&data[..self.size()])
    }
}

impl<'de> serde::Deserialize<'de> for Memory {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de> 
    {
        struct BytesVisitor;

        impl<'de> serde::de::Visitor<'de> for BytesVisitor
        {
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

                let capacity = v.len().next_power_of_two();

                let memory = Memory::with_capacity(capacity);
                memory.size.set(v.len());

                let data = unsafe {
                    std::slice::from_raw_parts_mut(memory.data, memory.capacity)
                };
                data[..v.len()].copy_from_slice(v);
        
                Ok(memory)
            }
        }

        deserializer.deserialize_bytes(BytesVisitor)
    }
}
