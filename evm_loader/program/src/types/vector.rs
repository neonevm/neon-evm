use crate::allocator::{acc_allocator, AccountAllocator};

pub type Vector<T> = allocator_api2::vec::Vec<T, AccountAllocator>;

#[macro_export]
macro_rules! vector {
    () => (
        allocator_api2::vec::Vec::new_in($crate::allocator::acc_allocator())
    );
    ($elem:expr; $n:expr) => (
        allocator_api2::vec::from_elem_in($elem, $n, $crate::allocator::acc_allocator())
    );
    ($($x:expr),+ $(,)?) => (
        allocator_api2::boxed::Box::<[_], $crate::allocator::AccountAllocator>::into_vec(
            allocator_api2::boxed::Box::slice(
                allocator_api2::boxed::Box::new_in([$($x),+], $crate::allocator::acc_allocator())
            )
        )
    );
}

pub fn into_vector<T>(v: Vec<T>) -> Vector<T> {
    let mut ret = Vector::with_capacity_in(v.len(), acc_allocator());
    for item in v {
        ret.push(item);
    }
    ret
}

