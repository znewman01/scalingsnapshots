use serde::{Serialize, Serializer};

#[derive(Debug, Serialize, Default)]
pub struct DataSize {
    bytes: u64,
}

impl DataSize {
    pub fn bytes(bytes: u64) -> Self {
        DataSize { bytes }
    }

    pub fn zero() -> Self {
        Self::default()
    }
}

pub fn data_size_as_bytes<S>(data_size: &DataSize, s: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    s.serialize_u64(data_size.bytes)
}

pub trait DataSized {
    fn size(&self) -> DataSize;
}

impl DataSized for () {
    fn size(&self) -> DataSize {
        DataSize::zero()
    }
}
