use serde::Serialize;
pub use uom::si::information::byte;

pub type Information = uom::si::u64::Information;

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
