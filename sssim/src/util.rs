use serde::{Serialize, Serializer};

#[derive(Debug, Serialize, Default)]
pub struct DataSize {
    bytes: u64,
}

impl DataSize {
    pub fn from_bytes(bytes: u64) -> Self {
        DataSize { bytes }
    }

    pub fn zero() -> Self {
        Self::default()
    }

    /// Get the data size's bytes.
    pub fn bytes(&self) -> u64 {
        self.bytes
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

impl<T: DataSized> DataSized for Option<T> {
    fn size(&self) -> DataSize {
        match self {
            None => DataSize::zero(),
            Some(inner) => inner.size(),
        }
    }
}

impl DataSized for rug::Integer {
    fn size(&self) -> DataSize {
        DataSize::from_bytes(self.significant_digits::<u8>().try_into().unwrap())
    }
}
