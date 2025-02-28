use {
    heed::{byteorder::LittleEndian, types::U64, BoxedError, BytesDecode, BytesEncode},
    moved_shared::primitives::B256,
    std::borrow::Cow,
};

#[derive(Debug)]
pub struct EncodableB256;

impl<'item> BytesEncode<'item> for EncodableB256 {
    type EItem = B256;

    fn bytes_encode(item: &'item Self::EItem) -> Result<Cow<'item, [u8]>, BoxedError> {
        Ok(Cow::Borrowed(item.as_slice()))
    }
}

impl<'item> BytesDecode<'item> for EncodableB256 {
    type DItem = B256;

    fn bytes_decode(bytes: &'item [u8]) -> Result<Self::DItem, BoxedError> {
        Ok(B256::try_from(bytes)?)
    }
}

pub type EncodableU64 = U64<LittleEndian>;
