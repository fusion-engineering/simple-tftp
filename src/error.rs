#[derive(Debug)]
pub enum Error {
    BufferTooSmall,
    InvalidOpcode(u16),
    InvalidAck,
    #[cfg(feature = "std")]
    IoError(std::io::Error),
}

pub type Result<T> = core::result::Result<T, Error>;
