#[cfg(feature = "storage-lmdb")]
pub use heed::*;
#[cfg(all(not(feature = "storage"), not(feature = "storage-lmdb")))]
pub use in_memory::*;
#[cfg(all(feature = "storage", not(feature = "storage-lmdb")))]
pub use rocksdb::*;

#[cfg(feature = "storage-lmdb")]
mod heed;
#[cfg(all(not(feature = "storage"), not(feature = "storage-lmdb")))]
mod in_memory;
#[cfg(all(feature = "storage", not(feature = "storage-lmdb")))]
mod rocksdb;
mod shared;
