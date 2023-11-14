use solana_program::slot_history::Slot;
use std::cmp::Ordering;

#[must_use]
pub fn find_slot_hash(value: Slot, slot_hashes_data: &[u8]) -> [u8; 32] {
    let slot_hashes_len = u64::from_le_bytes(slot_hashes_data[..8].try_into().unwrap());

    // copy-paste from slice::binary_search
    let mut size = usize::try_from(slot_hashes_len).unwrap() - 1;
    let mut left = 0;
    let mut right = size;

    while left < right {
        let mid = left + size / 2;
        let offset = mid * 40 + 8; // +8 - the first 8 bytes for the len of vector

        let slot = u64::from_le_bytes(slot_hashes_data[offset..][..8].try_into().unwrap());
        let cmp = value.cmp(&slot);

        // The reason why we use if/else control flow rather than match
        // is because match reorders comparison operations, which is perf sensitive.
        // This is x86 asm for u8: https://rust.godbolt.org/z/8Y8Pra.
        if cmp == Ordering::Less {
            left = mid + 1;
        } else if cmp == Ordering::Greater {
            right = mid;
        } else {
            return slot_hashes_data[(offset + 8)..][..32].try_into().unwrap();
        }

        size = right - left;
    }

    generate_fake_slot_hash(value)
}

#[must_use]
pub fn generate_fake_slot_hash(slot: Slot) -> [u8; 32] {
    let slot_bytes: [u8; 8] = slot.to_be_bytes();
    let mut initial = 0;
    for b in slot_bytes {
        if b != 0 {
            break;
        }
        initial += 1;
    }
    let slot_slice = &slot_bytes[initial..];
    let slot_slice_len = slot_slice.len();
    let mut hash = [255; 32];
    hash[32 - slot_slice_len - 1] = 0;
    hash[(32 - slot_slice_len)..].copy_from_slice(slot_slice);
    hash
}

#[test]
fn test_generate_fake_slot_hash() {
    let slot = 0x46;
    let mut expected: [u8; 32] = [255; 32];
    expected[30] = 0;
    expected[31] = 0x46;
    assert_eq!(generate_fake_slot_hash(slot), expected);

    let slot = 0x3e8;
    let mut expected: [u8; 32] = [255; 32];
    expected[29] = 0;
    expected[30] = 0x03;
    expected[31] = 0xe8;
    assert_eq!(generate_fake_slot_hash(slot), expected);
}
