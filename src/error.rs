/// Errors that can occur during TFTP parsing or io.
#[derive(Debug)]
pub enum Error {
    /// Buffer didn't have enough space for a packet
    BufferTooSmall,
    /// Packet had an invalid opcode.
    InvalidOpcode(u16),
    /// Received an ACK at the wrong time or with the wrong number
    InvalidAck,
    /// A stringy field was not formatted properly.
    BadFormatting,
    /// A packet contained the same option more than once
    OptionRepeated,
    /// packet had an invalid blocksize
    InvalidBlockSize(u32),
    #[cfg(feature = "std")]
    #[doc(cfg(feature = "std"))]
    /// an error occured during io
    IoError(std::io::Error),
}

/// Alias for `Result<T, Error>`
pub type Result<T> = core::result::Result<T, Error>;
