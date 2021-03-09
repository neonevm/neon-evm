use primitive_types::U256;
use arrayref::{array_ref, array_mut_ref, mut_array_refs};
use std::mem::size_of;

/*
#[derive(Debug)]
enum ProgramError {
    InvalidAccountData,
    AccountDataTooSmall,
}*/

/*
#[repr(C)]
#[derive(Serialize,Deserialize,Debug)]
struct HamtHeader {
    last_used: u32,
    free: [u32;32],
}*/

const ERR_ACCOUNTDATATOOSMALL: u8 = 0x20;
const ERR_INVALIDACCOUNTDATA: u8 = 0x40;

#[derive(Debug)]
pub struct Hamt<'a> {
    data: &'a mut [u8],
    //header: HamtHeader,
    last_used: u32,
    used: u32,
    item_count: u32,
}

enum ItemType {
    Empty,
    Item {pos: u32},
    Array {pos: u32},
}

impl<'a> Hamt<'a> {
    pub fn new(data: &'a mut [u8], reset: bool) -> Result<Self, u8> {
        let header_len = size_of::<u32>() * 32 * 2;

        if data.len() < header_len {
            return Err(ERR_ACCOUNTDATATOOSMALL);
        }

        if reset {
            data[0..header_len].copy_from_slice(&vec![0u8; header_len]);
            let last_used_ptr = array_mut_ref![data, 0, 4];
            *last_used_ptr = (header_len as u32).to_le_bytes();
            Ok(Hamt {data: data, last_used: header_len as u32, used: 0, item_count: 0})
        } else {
            let last_used_ptr = array_mut_ref![data, 0, 4];
            let last_used = u32::from_le_bytes(*last_used_ptr);
            if last_used < header_len as u32 {
                return Err(ERR_INVALIDACCOUNTDATA);
            }
            Ok(Hamt {data: data, last_used: last_used, used: 0, item_count: 0})
        }
    }

    fn allocate_item(&mut self, item_type: u8) -> Result<u32, u8> {
        let free_pos = item_type as u32 * size_of::<u32>() as u32;
        let size:u32 = match item_type {
            0 => (256+256)/8,
            _ => (4+item_type as u32 * 4),
        };
        if item_type < 32 && item_type > 0 {
            let item_pos = self.restore_u32(free_pos);
            if item_pos != 0 {
                let next_pos = self.restore_u32(item_pos);
                self.save_u32(free_pos, next_pos);
                self.used += size;
                return Ok(item_pos);
            }
        }
        if (self.last_used + size) as usize > self.data.len() {
            return Err(ERR_ACCOUNTDATATOOSMALL);
        }
        let item_pos = self.last_used;
        self.last_used += size;
        self.save_u32(0, self.last_used);
        self.used += size;
        Ok(item_pos)
    }

    fn release_item(&mut self, item_type: u8, item_pos: u32) {
        let free_pos = item_type as u32 * size_of::<u32>() as u32;
        if item_type >= 32 || item_type == 0 {panic!("Release unreleased items");};
        let size:u32 = match item_type {
            0 => (256+256)/8,
            _ => (4+item_type as u32 * 4),
        };
        self.save_u32(item_pos, self.restore_u32(free_pos));
        self.save_u32(free_pos, item_pos);
        self.used -= size;
    }

    fn place_item(&mut self, key: U256, value: U256) -> Result<u32, u8> {
        let pos = self.allocate_item(0)?;
        let ptr = array_mut_ref![self.data, pos as usize, 256/8*2];
        key.to_little_endian(&mut ptr[..256/8]);
        value.to_little_endian(&mut ptr[256/8..]);
        Ok(pos | 1)
    }

    fn place_items2(&mut self, tags: u32, item1: u32, item2: u32) -> Result<u32, u8> {
        let pos = self.allocate_item(2)?;
        let ptr = array_mut_ref![self.data, pos as usize, 3*4];
        let (tags_ptr, item1_ptr, item2_ptr) = mut_array_refs!(ptr, 4, 4, 4);
        *tags_ptr = tags.to_le_bytes();
        *item1_ptr = item1.to_le_bytes();
        *item2_ptr = item2.to_le_bytes();
        Ok(pos)
    }

    fn restore_value(&self, pos: u32) -> U256 {
        let ptr = array_ref![self.data, pos as usize, size_of::<U256>()];
        //println!("Restore value from {:x?}: {:x?}", pos, &ptr[..]);
        U256::from_little_endian(&ptr[..])
    }

    fn save_value(&mut self, pos: u32, value: &U256) {
        let ptr = array_mut_ref![self.data, pos as usize, size_of::<U256>()];
        value.to_little_endian(&mut ptr[..]);
    }

    fn restore_u32(&self, pos: u32) -> u32 {
        let ptr = array_ref![self.data, pos as usize, 4];
        u32::from_le_bytes(*ptr)
    }

    fn save_u32(&mut self, pos: u32, value: u32) {
        let ptr = array_mut_ref![self.data, pos as usize, 4];
        *ptr = value.to_le_bytes();
    }

    fn get_item(&self, pos: u32) -> ItemType {
        let d = self.restore_u32(pos);
        if d == 0 {
            return ItemType::Empty;
        }
        if d & 1 == 1 {
            return ItemType::Item {pos: d & !1};
        } else {
            return ItemType::Array {pos: d & !1};
        }
    }

    pub fn insert(&mut self, key: U256, value: U256) -> Result<(), u8> {
        let (key, tag) = (key >> 5, key.low_u32() & 0b11111);
        let ptr_pos = 32*4 + tag * 4;
        let res = self.insert_item(ptr_pos, key, value);
        if let Ok(_) = res {self.item_count += 1;};
        res
    }

    fn insert_item(&mut self, ptr_pos: u32, key: U256, value: U256) -> Result<(), u8> {
        match self.get_item(ptr_pos) {
            ItemType::Empty => {
                let item_pos = self.place_item(key, value)?;
                self.save_u32(ptr_pos, item_pos);
            },
            ItemType::Item{pos} => {
                let old_key = self.restore_value(pos);
                if old_key == key {
                    self.save_value(pos+size_of::<U256>() as u32, &value);
                    return Ok(());
                } else {
                    let mut ptr_pos = ptr_pos;
                    let (mut old_key, mut old_tag) = (old_key >> 5, old_key.low_u32() & 0b11111);
                    let (mut new_key, mut new_tag) = (key >> 5, key.low_u32() & 0b11111);
                    loop {
                        if old_tag != new_tag {break;}
                        let array_pos = self.allocate_item(1)?;

                        self.save_u32(array_pos, 1<<old_tag);
                        self.save_u32(ptr_pos, array_pos);
                        ptr_pos = array_pos+4;
                        old_tag = old_key.low_u32() & 0b11111; old_key = old_key >> 5;
                        new_tag = new_key.low_u32() & 0b11111; new_key = new_key >> 5;
                    }

                    let item_pos = self.place_item(new_key, value)?;
                    self.save_value(pos, &(old_key));

                    let tags = (1 << old_tag) | (1 << new_tag);
                    let (item1_pos, item2_pos) = if old_tag < new_tag {(pos|1, item_pos)} else {(item_pos, pos|1)};

                    let array_pos = self.place_items2(tags, item1_pos, item2_pos)?;
                    self.save_u32(ptr_pos, array_pos);
                }
                return Ok(());
            },
            ItemType::Array{pos} => {
                let (key, tag) = (key >> 5, key.low_u32() & 0b11111);
                let tags = self.restore_u32(pos);
                if tags & (1 << tag) == 0 {
                    // item with this tag doesn't exist in aray -> need resize
                    let total = tags.count_ones();
                    let shift = (tags & ((1 << tag)-1)).count_ones();
                    let (before_bytes, after_bytes) = (shift*4, (total-shift)*4);
                    let array_pos = self.allocate_item((total+1) as u8)?;
                    let item_pos = self.place_item(key, value)?;
                    self.save_u32(array_pos, tags | (1<<tag));
                    self.data.copy_within((pos+4) as usize..(pos+4+before_bytes) as usize, (array_pos+4) as usize);
                    self.save_u32(array_pos+4 + before_bytes, item_pos);
                    self.data.copy_within((pos+4+before_bytes) as usize..(pos+4+before_bytes+after_bytes) as usize, (array_pos+before_bytes+8) as usize);
                    self.release_item(total as u8, pos);
                    self.save_u32(ptr_pos, array_pos);
                } else {
                    // item with this tag already exist in array
                    let shift = (tags & ((1 << tag)-1)).count_ones();
                    return self.insert_item(pos+4 + shift*4, key, value);
                }
            },

        };
        Ok(())
    }

    pub fn find(&self, key: U256) -> Option<U256> {
        let (key, tag) = (key >> 5, key.low_u32() & 0b11111);
        let ptr_pos = 32*4 + tag * 4;
        self.find_item(ptr_pos, key)
    }

    fn find_item(&self, ptr_pos: u32, key: U256) -> Option<U256> {
        match self.get_item(ptr_pos) {
            ItemType::Empty => {
                return None;
            },
            ItemType::Item{pos} => {
                let old_key = self.restore_value(pos);
                if old_key == key {
                    Some(self.restore_value(pos+size_of::<U256>() as u32))
                } else {
                    return None;
                }
            },
            ItemType::Array{pos} => {
                let (key, tag) = (key >> 5, key.low_u32() & 0b11111);
                let tags = self.restore_u32(pos);
                if tags & (1 << tag) == 0 {
                    return None;
                } else {
                    let shift = (tags & ((1 << tag)-1)).count_ones();
                    return self.find_item(pos+4 + shift*4, key);
                }
            },
        }
    }
}
