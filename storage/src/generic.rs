/// The `ToKey` trait is designed to provide a unified way of encoding types to use as database
/// keys.
///
/// It is defined by a single operation [`Self::to_key`].
pub trait ToKey {
    /// Encodes the value as a key for [`rocksdb`].
    fn to_key(&self) -> impl AsRef<[u8]>;
}

pub trait ToValue {
    fn to_value(&self) -> Vec<u8>;
}

pub trait FromValue<'de> {
    fn from_value(slice: &'de [u8]) -> Self;
}

/// Implements the [`ToKey`] trait for an integer type.
macro_rules! int_impl {
    ($int:tt,$($types:tt)*) => {
        int_impl!($int);
        int_impl!($($types)*);
    };
    ($int:tt) => {
        impl ToKey for $int {
            fn to_key(&self) -> impl AsRef<[u8]> {
                self.to_be_bytes()
            }
        }
    };
}

int_impl!(u64);

impl<T: serde::Serialize> ToValue for T {
    fn to_value(&self) -> Vec<u8> {
        serde_json::to_vec(self).unwrap()
    }
}

impl<'de, T: serde::Deserialize<'de>> FromValue<'de> for T {
    fn from_value(slice: &'de [u8]) -> Self {
        serde_json::from_slice(slice).unwrap()
    }
}
