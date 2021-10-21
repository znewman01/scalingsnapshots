//! Adapters between Tuf metadata and log format.
use crate::log;
use crate::tuf;

impl From<tuf::FileName> for log::FileName {
    fn from(name: tuf::FileName) -> Self {
        let name: String = name.into();
        name.into()
    }
}

impl From<log::FileName> for tuf::FileName {
    fn from(name: log::FileName) -> Self {
        let name: String = name.into();
        name.into()
    }
}

impl From<tuf::File> for log::File {
    fn from(file: tuf::File) -> log::File {
        let length = file.length();
        log::File::new(file.name().into(), length)
    }
}

impl From<log::File> for tuf::File {
    fn from(file: log::File) -> tuf::File {
        let length = file.length();
        tuf::File::new(file.name().into(), length)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn log_filename_roundtrip(filename: log::FileName) {
            assert_eq!(
                log::FileName::from(tuf::FileName::from(filename.clone())),
                filename
            );
        }

        #[test]
        fn tuf_filename_roundtrip(filename: tuf::FileName) {
            assert_eq!(
                tuf::FileName::from(log::FileName::from(filename.clone())),
                filename
            );
        }

        #[test]
        fn log_file_roundtrip(file: log::File) {
            assert_eq!(log::File::from(tuf::File::from(file.clone())), file);
        }

        #[test]
        fn tuf_file_roundtrip(file: tuf::File) {
            assert_eq!(tuf::File::from(log::File::from(file.clone())), file);
        }
    }
}
