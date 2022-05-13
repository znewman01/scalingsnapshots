use serde::Serialize;
use uom::si::information::byte;

type Information = uom::si::u64::Information;

pub trait DataSized {
    fn size(&self) -> Information;
}

// TODO: we should probably have a derive macro for this
impl<T: Serialize> DataSized for T {
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
