use serde::Serialize;
pub use uom::si::information::byte;

pub type Information = uom::si::u64::Information;

// assumed average size of all package name strings
pub static STRING_BYTES: usize = 8;

pub trait DataSized {
    fn size(&self) -> Information;
}

pub trait DataSizeFromSerialize: Serialize {}

impl<T: DataSizeFromSerialize> DataSized for T {
    fn size(&self) -> Information {
        Information::new::<byte>(
            serde_json::to_string(self)
                .expect("serialization should work")
                .len()
                .try_into()
                .expect("not that big"),
        )
    }
}

impl DataSized for () {
    fn size(&self) -> Information {
        uom::ConstZero::ZERO
    }
}

impl DataSized for rug::Integer {
    fn size(&self) -> Information {
        Information::new::<byte>(self.significant_digits::<u8>().try_into().unwrap())
    }
}

impl DataSized for u64 {
    fn size(&self) -> Information {
        Information::new::<byte>(*self)
    }
}
