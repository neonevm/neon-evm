use crate::allocator::{acc_allocator, AccountAllocator};

pub type Boxx<T> = allocator_api2::boxed::Box<T, AccountAllocator>;

pub fn boxx<T>(value: T) -> Boxx<T> {
    Boxx::new_in(value, acc_allocator())
}