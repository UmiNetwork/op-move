use std::{collections::HashMap, hash::Hash};

/// The trait `GroupIterator` is defined by a single [`Self::group_by`] operation.
pub trait GroupIterator<V> {
    /// Consumes the iterator and allocates a [`HashMap`].
    ///
    /// The iterator can extract key [`K`] by passing the function `f` from a reference to value
    /// [`V`]. Two different key-value pairs with equal keys are considered as having duplicate keys.
    ///
    /// The result of this operation is a [`HashMap`] where keys are taken from key-value pairs.
    /// For a key [`K`] a value is a vector of every [`V`] associated with the key in the iterator.
    ///
    /// The keys of type [`K`] are compared using their [`Eq`] implementation and hashed by their
    /// [`Hash`] implementation. The values [`V`] have no bounds.
    ///
    /// This operation is often referred to as "grouping by key," hence the name.
    fn group_by<K: Eq + Hash, F: Fn(&V) -> K>(self, f: F) -> HashMap<K, Vec<V>>;
}

impl<V, I: Iterator<Item = V>> GroupIterator<V> for I {
    fn group_by<K: Eq + Hash, F: Fn(&V) -> K>(self, f: F) -> HashMap<K, Vec<V>> {
        let mut hash_map = match self.size_hint() {
            (_, Some(len)) => HashMap::with_capacity(len),
            (len, None) => HashMap::with_capacity(len),
        };

        for (key, value) in self.map(|v| (f(&v), v)) {
            hash_map
                .entry(key)
                .or_insert_with(|| Vec::with_capacity(1))
                .push(value)
        }

        hash_map
    }
}
