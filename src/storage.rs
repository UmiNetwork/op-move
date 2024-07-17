use {
    move_binary_format::errors::PartialVMError,
    move_core_types::{effects::ChangeSet, resolver::MoveResolver},
    move_vm_test_utils::InMemoryStorage,
    std::fmt::Debug,
};

/// A persistent storage trait.
///
/// This trait inherits [`MoveResolver`] that can resolve both resources and modules and extends it
/// with the [`apply`] operation.
///
/// [`apply`]: Self::apply
pub trait Storage: MoveResolver<Self::Err> {
    /// The associated error that can occur on storage operations.
    type Err: Debug;

    /// Applies the `changes` to the underlying storage state.
    fn apply(&mut self, changes: ChangeSet) -> Result<(), Self::Err>;
}

impl Storage for InMemoryStorage {
    type Err = PartialVMError;

    fn apply(&mut self, changes: ChangeSet) -> Result<(), PartialVMError> {
        InMemoryStorage::apply(self, changes)
    }
}
