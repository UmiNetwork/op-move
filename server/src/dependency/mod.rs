#[cfg(not(feature = "storage"))]
pub use in_memory::*;
#[cfg(feature = "storage")]
pub use rocksdb::*;

#[cfg(not(feature = "storage"))]
mod in_memory;
#[cfg(feature = "storage")]
mod rocksdb;
mod shared;
