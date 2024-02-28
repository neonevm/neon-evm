use std::{
    fmt::{self, Debug, Display},
    hash::Hash,
    iter::Zip,
    usize,
};

use crate::allocator::acc_allocator;
use super::Vector;

pub struct TreeMap<K, V> {
    keys: Vector<K>,
    values: Vector<V>,
}

impl<K: Ord, V> TreeMap<K, V> {
    pub fn new() -> Self {
        TreeMap {
            keys: Vector::new_in(acc_allocator()),
            values: Vector::new_in(acc_allocator()),
        }
    }

    pub fn with_capacity(capacity: usize) -> Self {
        TreeMap {
            keys: Vector::with_capacity_in(capacity, acc_allocator()),
            values: Vector::with_capacity_in(capacity, acc_allocator()),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.keys.is_empty()
    }

    pub fn len(&self) -> usize {
        self.keys.len()
    }

    pub fn get(&self, key: &K) -> Option<&V> {
        match self.keys.binary_search(&key) {
            Ok(idx) => Option::Some(&self.values[idx]),
            Err(_) => Option::None,
        }
    }

    pub fn get_mut(&mut self, key: &K) -> Option<&mut V> {
        match self.keys.binary_search(&key) {
            Ok(_idx) => Option::Some(&mut self.values[_idx]),
            Err(_idx) => Option::None,
        }
    }

    pub fn insert(&mut self, key: K, value: &V) -> Option<V>
    where
        V: Clone,
    {
        match self.keys.binary_search(&key) {
            Ok(idx) => {
                // Clone is better in performance than potential vec realloc.
                let old = self.values[idx].clone();
                self.values.insert(idx, value.clone());
                Some(old)
            }
            Err(idx) => {
                self.keys.insert(idx, key);
                self.values.insert(idx, value.clone());
                None
            }
        }
    }

    pub fn remove(&mut self, key: K) -> Option<V> {
        match self.keys.binary_search(&key) {
            Ok(idx) => {
                self.keys.remove(idx);
                Some(self.values.remove(idx))
            }
            Err(_) => None,
        }
    }

    pub fn remove_entry(&mut self, key: K) -> Option<(K, V)> {
        match self.keys.binary_search(&key) {
            Ok(idx) => Some((self.keys.remove(idx), self.values.remove(idx))),
            Err(_) => None,
        }
    }

    pub fn keys(&self) -> impl Iterator<Item= &K> {
        self.keys.iter()
    }
}

impl<'a, K: 'a, V: 'a> TreeMap<K, V> {
    pub fn iter(&'a self) -> Zip<std::slice::Iter<'a, K>, std::slice::Iter<'a, V>> {
        std::iter::zip(self.keys.iter(), self.values.iter())
    }

    pub fn iter_mut(&'a mut self) -> Zip<std::slice::IterMut<'a, K>, std::slice::IterMut<'a, V>> {
        std::iter::zip(self.keys.iter_mut(), self.values.iter_mut())
    }
}

impl<K: Debug, V: Debug> fmt::Debug for TreeMap<K, V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut res = write!(f, "TreeMap {{");
        for i in 0..self.keys.len() {
            res = res.and(write!(f, "{:?} -> {:?}, ", self.keys[i], self.values[i]));
        }
        res.and(write!(f, " }}"))
    }
}

impl<K: Display, V: Display> fmt::Display for TreeMap<K, V> {
    // This trait requires `fmt` with this exact signature.
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut res = write!(f, "TreeMap {{");
        for i in 0..self.keys.len() {
            res = res.and(write!(f, "{} -> {}, ", self.keys[i], self.values[i]));
        }
        res.and(write!(f, " }}"))
    }
}

impl<K: Hash, V: Hash> Hash for TreeMap<K, V> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.keys.hash(state);
        self.values.hash(state);
    }
}
