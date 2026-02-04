use {
    crate::diff::AllAccountChanges,
    bytes::Bytes,
    move_binary_format::errors::{PartialVMResult, VMResult},
    move_core_types::{
        account_address::AccountAddress, identifier::IdentStr, language_storage::StructTag,
        metadata::Metadata, value::MoveTypeLayout,
    },
    move_vm_types::{code::ModuleBytesStorage, resolver::ResourceResolver},
};

/// Resolver derived from a pair of existing resolvers.
/// It tries to look up the module/resource with the primary resolver first,
/// trying the secondary if that lookup fails. Note: this means if the same
/// key is present is both then the primary overshadows the secondary.
/// Note: "fails" includes returning `Ok(None)` and `Err(_)`, this means if
/// the primary resolver is broken then the secondary resolver is effectively
/// all that remains. It also means that if both resolvers are broken then the
/// error from the secondary resolver is returned in the Result.
pub struct PairedResolvers<'a, T, U> {
    primary: &'a T,
    secondary: &'a U,
}

impl<'a, T, U> PairedResolvers<'a, T, U> {
    pub fn new(primary: &'a T, secondary: &'a U) -> Self {
        Self { primary, secondary }
    }
}

impl<T: ResourceResolver, U: ResourceResolver> ResourceResolver for PairedResolvers<'_, T, U> {
    fn get_resource_bytes_with_metadata_and_layout(
        &self,
        address: &AccountAddress,
        struct_tag: &StructTag,
        metadata: &[Metadata],
        layout: Option<&MoveTypeLayout>,
    ) -> PartialVMResult<(Option<Bytes>, usize)> {
        let primary_result = self
            .primary
            .get_resource_bytes_with_metadata_and_layout(address, struct_tag, metadata, layout);
        match primary_result {
            Ok((Some(bytes), size)) => Ok((Some(bytes), size)),
            Ok((None, _)) | Err(_) => self
                .secondary
                .get_resource_bytes_with_metadata_and_layout(address, struct_tag, metadata, layout),
        }
    }
}

impl<T: ModuleBytesStorage, U: ModuleBytesStorage> ModuleBytesStorage
    for PairedResolvers<'_, T, U>
{
    fn fetch_module_bytes(
        &self,
        address: &AccountAddress,
        module_name: &IdentStr,
    ) -> VMResult<Option<Bytes>> {
        let primary_result = self.primary.fetch_module_bytes(address, module_name);
        match primary_result {
            Ok(Some(bytes)) => Ok(Some(bytes)),
            Ok(None) | Err(_) => self.secondary.fetch_module_bytes(address, module_name),
        }
    }
}

/// Resolver which looks up resources and modules based on a given change set.
pub struct ChangesBasedResolver<'a> {
    changes: &'a AllAccountChanges,
}

impl<'a> ChangesBasedResolver<'a> {
    pub fn new(changes: &'a AllAccountChanges) -> Self {
        Self { changes }
    }
}

impl ResourceResolver for ChangesBasedResolver<'_> {
    fn get_resource_bytes_with_metadata_and_layout(
        &self,
        address: &AccountAddress,
        struct_tag: &StructTag,
        _metadata: &[Metadata],
        _layout: Option<&MoveTypeLayout>,
    ) -> PartialVMResult<(Option<Bytes>, usize)> {
        let bytes = self
            .changes
            .accounts()
            .get(address)
            .and_then(|account| account.resources().get(struct_tag))
            .and_then(|op| op.clone().ok());
        let size = bytes.as_ref().map(|b| b.len()).unwrap_or(0);
        Ok((bytes, size))
    }
}

impl ModuleBytesStorage for ChangesBasedResolver<'_> {
    fn fetch_module_bytes(
        &self,
        address: &AccountAddress,
        module_name: &IdentStr,
    ) -> VMResult<Option<Bytes>> {
        let bytes = self
            .changes
            .accounts()
            .get(address)
            .and_then(|account| account.modules().get(module_name))
            .and_then(|op| op.clone().ok());
        Ok(bytes)
    }
}
