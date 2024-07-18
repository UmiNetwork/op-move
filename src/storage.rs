use {
    move_binary_format::errors::PartialVMError,
    move_core_types::{effects::ChangeSet, resolver::MoveResolver},
    move_table_extension::TableChangeSet,
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

    /// Applies the `changes` to the underlying storage state. In addition, applies `table_changes`
    /// using the [`move_table_extension`].
    ///
    /// Requires `table-extension` feature. The method exists even with this feature being turned
    /// off to allow using it whether the feature flag is on or not. When off, it behaves like
    /// [`Storage::apply`]
    fn apply_with_tables(
        &mut self,
        changes: ChangeSet,
        #[cfg(feature = "table-extension")] table_changes: TableChangeSet,
    ) -> Result<(), Self::Err>;
}

impl Storage for InMemoryStorage {
    type Err = PartialVMError;

    fn apply(&mut self, changes: ChangeSet) -> Result<(), PartialVMError> {
        InMemoryStorage::apply(self, changes)
    }

    fn apply_with_tables(
        &mut self,
        changes: ChangeSet,
        #[cfg(feature = "table-extension")] table_changes: TableChangeSet,
    ) -> Result<(), PartialVMError> {
        InMemoryStorage::apply_extended(self, changes, table_changes)
    }
}
