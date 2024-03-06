use crate::allocator::{acc_allocator, StateAccountAllocator};

pub type Boxx<T> = allocator_api2::boxed::Box<T, StateAccountAllocator>;

pub fn boxx<T>(value: T) -> Boxx<T> {
    Boxx::new_in(value, acc_allocator())
}