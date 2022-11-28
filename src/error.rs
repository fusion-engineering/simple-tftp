use std::io::Error as IoError;

#[derive(Debug)]
pub enum Error {
    BufferTooSmall,
    InvalidOpcode(u16),
    InvalidAck,
    IoError(IoError),
}

pub type Result<T> = std::result::Result<T, Error>;
