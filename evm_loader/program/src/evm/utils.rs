pub fn checked_next_multiple_of_32(n: usize) -> Option<usize> {
    Some(n.checked_add(31)? & !31)
}
