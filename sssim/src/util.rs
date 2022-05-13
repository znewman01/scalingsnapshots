use serde::{Serialize, Serializer};

#[derive(Debug, Serialize, Default)]
pub struct DataSize {
    bytes: u64,
}

impl DataSize {
    #[must_use]
    pub fn from_bytes(bytes: u64) -> Self {
        DataSize { bytes }
    }

    #[must_use]
    pub fn zero() -> Self {
        Self::default()
    }

    /// Get the data size's bytes.
    #[must_use]
    pub fn bytes(&self) -> u64 {
        self.bytes
    }
}

impl From<usize> for DataSize {
    fn from(value: usize) -> Self {
        DataSize::from_bytes(value.try_into().unwrap())
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

impl<T: Serialize> DataSized for T {
    fn size(&self) -> DataSize {
        serde_json::to_string(self)
            .expect("serialization should work")
            .len()
            .into()
    }
}
