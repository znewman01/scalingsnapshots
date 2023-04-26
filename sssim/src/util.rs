use std::collections::HashMap;

pub use uom::si::information::byte;
use uom::ConstZero;

pub type Information = uom::si::usize::Information;

pub trait DataSized {
    fn size(&self) -> Information;
}

pub trait FixedDataSized {
    fn fixed_size() -> Information;
}

impl<T: FixedDataSized> DataSized for T {
    fn size(&self) -> Information {
        T::fixed_size()
    }
}

impl<T: FixedDataSized> DataSized for Vec<T> {
    fn size(&self) -> Information {
        T::fixed_size() * self.len()
    }
}

/// Compute a data size for the given `Vec` based on the first element.
pub fn assume_data_size_for_vec<T: DataSized>(vec: &Vec<T>) -> Information {
    vec.len()
        * match vec.get(0) {
            Some(x) => x.size(),
            None => Information::ZERO,
        }
}

impl<T: FixedDataSized, U: FixedDataSized> DataSized for HashMap<T, U> {
    fn size(&self) -> Information {
        (T::fixed_size() + U::fixed_size()) * self.len()
    }
}

/// Compute a data size for the given `HashMap` based on the first element.
pub fn assume_data_size_for_map<T: DataSized, U: DataSized>(map: &HashMap<T, U>) -> Information
where
{
    map.len()
        * match map.iter().next() {
            Some((k, v)) => k.size() + v.size(),
            None => Information::ZERO,
        }
}

impl DataSized for () {
    fn size(&self) -> Information {
        Information::ZERO
    }
}

impl DataSized for rug::Integer {
    fn size(&self) -> Information {
        Information::new::<byte>(self.significant_digits::<u8>())
    }
}

impl FixedDataSized for u8 {
    fn fixed_size() -> Information {
        Information::new::<byte>(1)
    }
}

impl FixedDataSized for u32 {
    fn fixed_size() -> Information {
        Information::new::<byte>(4)
    }
}

impl FixedDataSized for u64 {
    fn fixed_size() -> Information {
        Information::new::<byte>(8)
    }
}

impl FixedDataSized for usize {
    fn fixed_size() -> Information {
        Information::new::<byte>(8)
    }
}

impl<T: DataSized, U: DataSized> DataSized for (T, U) {
    fn size(&self) -> Information {
        self.0.size() + self.1.size()
    }
}

impl<T: DataSized> DataSized for Option<T> {
    fn size(&self) -> Information {
        match self {
            Some(t) => t.size(),
            None => uom::ConstZero::ZERO,
        }
    }
}
