use std::{collections::HashMap, hash::Hash};

/// The trait `GroupIterator` is defined by a single [`Self::group`] operation.
pub trait GroupIterator<K: Eq + Hash, V> {
    /// Consumes the iterator and allocates a [`HashMap`].
    ///
    /// The iterator can define multiple values [`V`] paired with a key of type [`K`]. Two different
    /// key-value pairs where keys are equal are considered as having duplicate keys.
    ///
    /// The result of this operation is a [`HashMap`] where keys are taken from key-value pairs.
    /// For a key [`K`] a value is a vector of every [`V`] associated with the key in the iterator.
    ///
    /// The keys of type [`K`] are compared using their [`Eq`] implementation and hashed by their
    /// [`Hash`] implementation. The values [`V`] have no bounds.
    ///
    /// This operation is often referred to as "grouping by key," hence the name.
    fn group(self) -> HashMap<K, Vec<V>>;
}

impl<K: Eq + Hash, V, I: Iterator<Item = (K, V)>> GroupIterator<K, V> for I {
    fn group(self) -> HashMap<K, Vec<V>> {
        let mut hash_map = match self.size_hint() {
            (_, Some(len)) => HashMap::with_capacity(len),
            (len, None) => HashMap::with_capacity(len),
        };

        for (key, value) in self {
            hash_map
                .entry(key)
                .or_insert_with(|| Vec::with_capacity(1))
                .push(value)
        }

        hash_map
    }
}
