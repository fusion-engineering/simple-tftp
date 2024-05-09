use crate::error::{Error as TftpError, Result as TftpResult};
use core::{fmt::Write, num::NonZeroU8};

struct BufferWriter<'a> {
    buff: &'a mut [u8],
    size: usize,
    overflowed: bool,
}

impl<'a> BufferWriter<'a> {
    pub fn new(buff: &'a mut [u8]) -> Self {
        Self {
            buff,
            size: 0,
            overflowed: false,
        }
    }

    pub fn push_bytes(&mut self, bytes: &[u8]) {
        let free_bytes = self.buff.len() - self.size;
        let to_push = bytes.len().min(free_bytes);
        self.buff[self.size..(self.size + to_push)].copy_from_slice(&bytes[..to_push]);
        self.size += to_push;
        if to_push < bytes.len() {
            self.overflowed = true;
        }
    }

    pub fn push_byte(&mut self, byte: u8) {
        if self.size < self.buff.len() {
            self.buff[self.size] = byte;
            self.size += 1;
        } else {
            self.overflowed = true;
        }
    }

    pub fn overflowed(&self) -> bool {
        self.overflowed
    }
}

impl<'a> core::fmt::Write for BufferWriter<'a> {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        self.push_bytes(s.as_bytes());
        if self.overflowed() {
            Err(core::fmt::Error)
        } else {
            Ok(())
        }
    }
}

/// The 16 bit opcodes used for TFTP packets as defined in [RFC-1350](https://www.rfc-editor.org/rfc/inline-errata/rfc1350.html) section 5 and [RFC-2347](https://www.rfc-editor.org/rfc/inline-errata/rfc2347.html).
#[derive(Debug, Eq, PartialEq)]
#[repr(u16)]
#[allow(missing_docs)]
pub enum OpCode {
    ReadRequest = 1,
    WriteRequest = 2,
    Data = 3,
    Acknowledgement = 4,
    Error = 5,
    OptionAck = 6,
}

impl TryFrom<u16> for OpCode {
    type Error = TftpError;
    fn try_from(value: u16) -> Result<Self, <Self as TryFrom<u16>>::Error> {
        match value {
            1 => Ok(Self::ReadRequest),
            2 => Ok(Self::WriteRequest),
            3 => Ok(Self::Data),
            4 => Ok(Self::Acknowledgement),
            5 => Ok(Self::Error),
            6 => Ok(Self::OptionAck),
            e => Err(TftpError::InvalidOpcode(e)),
        }
    }
}

/// Error codes as defined in the apendix of [RFC-1350](https://www.rfc-editor.org/rfc/inline-errata/rfc1350.html).
///
/// Note that these are not exhaustive. When generating an error message that is not defined here, you should
/// use NOT_DEFINED with a custom error message. The [[Error]] packet can be constructed with any error code however.
/// In order to let it deal with non-compliant end-points, or newer standards.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ErrorCode(u16);
impl ErrorCode {
    /// When implementing an endpoint, use this error code to send custom error messages
    /// that will make sure your implementation won't conflict with any future additions to this list
    pub const NOT_DEFINED: Self = Self(0);
    /// File not found.
    pub const FILE_NOT_FOUND: Self = Self(1);
    /// Access violation.
    pub const ACCESS_VIOLATION: Self = Self(2);
    /// Disk full or allocation exceeded.
    pub const DISK_FULL_OR_ALLOCATION_EXCEEDED: Self = Self(3);
    /// Illegal TFTP operation.
    pub const ILLEGAL_TFTP_OPERATION: Self = Self(4);
    /// Unknown transfer ID.
    pub const UNKNOWN_TRANSFER_ID: Self = Self(5);
    /// File already exists.
    pub const FILE_ALREADY_EXISTS: Self = Self(6);
    /// No such user.
    pub const NO_SUCH_USER: Self = Self(7);
    fn possibly_invalid(code: u16) -> Self {
        Self(code)
    }
}

impl core::fmt::Display for ErrorCode {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match *self {
            Self::NOT_DEFINED => f.write_str("Not defined, see error message (if any)"),
            Self::FILE_NOT_FOUND => f.write_str("File not found"),
            Self::ACCESS_VIOLATION => f.write_str("Access violation"),
            Self::DISK_FULL_OR_ALLOCATION_EXCEEDED => {
                f.write_str("Disk full or allocation exceeded")
            }
            Self::ILLEGAL_TFTP_OPERATION => f.write_str("Illegal TFTP operation"),
            Self::UNKNOWN_TRANSFER_ID => f.write_str("Unknown transfer ID"),
            Self::FILE_ALREADY_EXISTS => f.write_str("File already exists"),
            Self::NO_SUCH_USER => f.write_str("No such user"),
            _ => f.write_fmt(format_args!("Undefined Error Code({})", self.0)),
        }
    }
}

/// A read- or write-request packet.
///
/// Will always use the octer mode. netascii mode is not supported.
#[derive(Debug)]
pub struct Request<'a> {
    is_read: bool,
    /// the requested filename. Should be in net-ascii according to the standard but we support utf-8.
    pub filename: &'a str,
    //only the octet mode is supported so it isn't stored here
    /// The blocksize requested using the options extension defined in [RFC-2348](https://www.rfc-editor.org/rfc/rfc2348.html).
    pub blocksize: Option<u16>,
    /// If set, the packet will send the size of the file should be to the server (on a write request) or request the file size from the server (on a read request) using the tsize option defined in [RFC-2349](https://www.rfc-editor.org/rfc/rfc2349.html)
    pub include_transfer_size: bool,
    /// unsupported, see [RFC-2349](https://www.rfc-editor.org/rfc/rfc2349.html) for a definition
    pub timeout_seconds: Option<NonZeroU8>,
    unknown_options: &'a [u8],
}

/// A data package that borrows a slice of data
#[derive(Debug)]
pub struct Data<'a> {
    block_nr: u16,
    data: &'a [u8],
}

/// an acknowledge packet, send in response to a data packet
#[derive(Debug)]
pub struct Ack {
    /// the block_nr of the data packet being ack'ed.
    /// A write request is acked with a block_nr of 0.
    pub block_nr: u16,
}

/// An error packet
#[derive(Debug)]
pub struct Error<'a> {
    /// The specific error code. See [`ErrorCode`] for details.
    pub error_code: ErrorCode,
    // todo: should be (net)ascii?
    /// The human read-able error message associated with this string.
    /// Note that as per the spec, it should be ascii:
    /// > The error message is intended for human consumption, and
    /// > should be in netascii.  Like all other strings, it is terminated with
    /// > a zero byte.
    pub message: &'a str,
}

/// an option acknowledge packet
///
/// These are send in response to a read or write request to confirm which optional extension to use for the transfer.
#[derive(Debug)]
pub struct OptionAck<'a> {
    /// Indicates acknowledgement of a specific blocksize requested using the options extension defined in [RFC-2348](https://www.rfc-editor.org/rfc/rfc2348.html) if present.
    pub blocksize: Option<u16>,
    /// If set, indicates the acknowledgement of the tsize options extension as defined in [RFC-2349](https://www.rfc-editor.org/rfc/rfc2349.html).
    /// On a read request, the value of the field will be set to the size of the requested file. On a write request it will echo back the size reported by the client.
    /// If the packet is too larger, either side may abort the transfer with an [Error] packet with code [`ErrorCode::DISK_FULL_OR_ALLOCATION_EXCEEDED`].
    pub transfer_size: Option<u64>,
    /// If set, indicates acknowledgement of timeour option extension as defined in [RFC-2349](https://www.rfc-editor.org/rfc/rfc2349.html)
    pub timeout_seconds: Option<NonZeroU8>,
    /// options which aren't understood by this library
    unknown_options: &'a [u8],
}

/// an enum of all types of TFTP packet
#[derive(Debug)]
pub enum Packet<'a> {
    /// A data package that borrows a slice of data,
    Data(Data<'a>),
    /// A read- or write-request packet,
    Request(Request<'a>),
    /// An error packet indicating something went wrong,
    Error(Error<'a>),
    /// An acknowledge packet, send in response to a data packet,
    Ack(Ack),
    /// An option acknowledge packet, acknowledging options request with a read- or write-request packet,
    OptionAck(OptionAck<'a>),
}

impl<'a> Packet<'a> {
    /// creates a new data packet with the given `block_nr` and `data`.
    pub fn new_data(block_nr: u16, data: &'a [u8]) -> Self {
        Self::Data(Data::new(block_nr, data))
    }

    /// creates a packet that request to read the file `filename` in chunks of `blocksize` bytes.
    /// if `blocksize` is `None` no specific blocksize is requested from the end-point and 512 should be used as the default blocksize
    pub fn new_read_request(filename: &'a str, blocksize: Option<u16>) -> Self {
        Self::Request(Request::new_read_request(filename, blocksize))
    }

    /// creates a packet that request to write the file `filename` in chunks of `blocksize` bytes.
    /// if `blocksize` is `None` no specific blocksize is requested from the end-point and 512 should be used as the default blocksize
    pub fn new_write_request(filename: &'a str, blocksize: Option<u16>) -> Self {
        Self::Request(Request::new_write_request(filename, blocksize))
    }

    /// creates an error packet with the given code and message.
    pub fn new_error(error_code: ErrorCode, message: &'a str) -> Self {
        Self::Error(Error::new(error_code, message))
    }

    /// creates an acknowledge packet for data packet number `block_nr`.
    pub fn new_ack(block_nr: u16) -> Self {
        Self::Ack(Ack::new(block_nr))
    }

    /// creates a packet from a data buffer.
    /// the first two bytes should be the 16-byte big-endian opcode.
    /// the buffer is allowed to be bigger than the TFTP packet.
    pub fn from_bytes(data: &'a [u8]) -> TftpResult<Self> {
        if data.len() < 2 {
            return Err(TftpError::BufferTooSmall);
        }

        OpCode::try_from(u16::from_be_bytes([data[0], data[1]])).map(|opcode| {
            Ok(match opcode {
                OpCode::ReadRequest => {
                    Self::Request(Request::from_bytes_skip_opcode_check(data, true)?)
                }
                OpCode::WriteRequest => {
                    Self::Request(Request::from_bytes_skip_opcode_check(data, false)?)
                }
                OpCode::Data => Self::Data(Data::from_bytes_skip_opcode_check(data)?),
                OpCode::Acknowledgement => Self::Ack(Ack::from_bytes_skip_opcode_check(data)?),
                OpCode::Error => Self::Error(Error::from_bytes_skip_opcode_check(data)?),
                OpCode::OptionAck => {
                    Self::OptionAck(OptionAck::from_bytes_skip_opcode_check(data)?)
                }
            })
        })?
    }

    /// returns the opcode of the packet.
    pub fn opcode(&self) -> OpCode {
        match self {
            Self::Ack(_) => OpCode::Acknowledgement,
            Self::Data(_) => OpCode::Data,
            Self::Error(_) => OpCode::Error,
            Self::Request(Request { is_read: true, .. }) => OpCode::ReadRequest,
            Self::Request(Request { is_read: false, .. }) => OpCode::WriteRequest,
            Self::OptionAck(_) => OpCode::OptionAck,
        }
    }

    /// write this packet into the buffer `data`. The buffer is allowed to be larger than the packet size.
    /// Will return [TftpError::BufferTooSmall] if the packet doesn't fit but might still mutate the buffer.
    pub fn to_bytes(&self, data: &'a mut [u8]) -> Result<usize, TftpError> {
        match self {
            Self::Ack(x) => x.to_bytes(data),
            Self::Data(x) => x.to_bytes(data),
            Self::Error(x) => x.to_bytes(data),
            Self::Request(x) => x.to_bytes(data),
            Self::OptionAck(x) => x.to_bytes(data),
        }
    }
}

impl<'a> Data<'a> {
    /// creates a new data packet with the given block number and data.
    pub fn new(block_nr: u16, data: &'a [u8]) -> Self {
        Self { block_nr, data }
    }

    fn from_bytes_skip_opcode_check(data: &'a [u8]) -> TftpResult<Self> {
        if data.len() < 4 {
            return Err(TftpError::BufferTooSmall);
        }
        let block_nr = u16::from_be_bytes([data[2], data[3]]);
        let data = &data[4..];
        Ok(Self { block_nr, data })
    }

    /// write this packet into the buffer `data`. The buffer is allowed to be larger than the packet size.
    /// Will return [TftpError::BufferTooSmall] if the packet doesn't fit but might still mutate the buffer.
    pub fn to_bytes(&self, buf: &'a mut [u8]) -> Result<usize, TftpError> {
        let n_bytes = 4 + self.data.len();
        if n_bytes > buf.len() {
            return Err(TftpError::BufferTooSmall);
        }
        buf[0..2].copy_from_slice(&(OpCode::Data as u16).to_be_bytes());
        buf[2..4].copy_from_slice(&self.block_nr.to_be_bytes());
        buf[4..n_bytes].copy_from_slice(self.data);
        Ok(n_bytes)
    }
}

// todo: PR alterinative from_bytes functions for Cstr, possibly include direct str conversion.
// The data is supposed to be netascii, a really outdated format. no good reason to limit ourselves to it aside from "the standard says so"
// and this function will usually be called on data generated by a remote host, which may not be compliant itself
// and instead send utf-8 or 'normal' ascii.
fn printable_ascii_str_from_u8(data: &[u8]) -> TftpResult<(&str, &[u8])> {
    let first_non_ascii = data.into_iter().position(|&n| n < 32 || n > 127);
    if let Some(index) = first_non_ascii {
        if data[index] == 0 {
            return Ok(unsafe {
                (
                    core::str::from_utf8_unchecked(&data[..index]),
                    &data[(index + 1).min(data.len())..],
                )
            });
        }
    }
    Err(TftpError::BadFormatting)
}

fn get_option_pair(data: &[u8]) -> TftpResult<Option<((&str, &str), &[u8])>> {
    if data.len() == 0 {
        Ok(None)
    } else {
        let (name, data) = printable_ascii_str_from_u8(data)?;
        let (value, data) = printable_ascii_str_from_u8(data)?;
        Ok(Some(((name, value), data)))
    }
}

fn parse_blocksize(as_str: &str) -> TftpResult<u16> {
    let Ok(requested_blocksize) = as_str.parse::<u32>() else {
        return Err(TftpError::BadFormatting);
    };
    //Valid values range between "8" and "65464" octets, inclusive.
    if requested_blocksize < 8 || requested_blocksize > 65464 {
        Err(TftpError::InvalidBlockSize(requested_blocksize))
    } else {
        Ok(requested_blocksize as u16)
    }
}

impl<'a> Request<'a> {
    /// creates a new read request packet for the given file, optionally request a specific blocksize using the blocksize option defined in [RFC-2347](https://www.rfc-editor.org/rfc/inline-errata/rfc2347.html) and [RFC-2348](https://www.rfc-editor.org/rfc/rfc2348.html)
    pub fn new_read_request(filename: &'a str, blocksize: Option<u16>) -> Self {
        Self::new_request(filename, blocksize, true)
    }

    /// creates a new write request packet for the given file, optionally request a specific blocksize using the blocksize option defined in [RFC-2347](https://www.rfc-editor.org/rfc/inline-errata/rfc2347.html) and [RFC-2348](https://www.rfc-editor.org/rfc/rfc2348.html)
    pub fn new_write_request(filename: &'a str, blocksize: Option<u16>) -> Self {
        Self::new_request(filename, blocksize, false)
    }

    /// creates a new read or write request packet for the given file, optionally request a specific blocksize using the blocksize option defined in [RFC-2347](https://www.rfc-editor.org/rfc/inline-errata/rfc2347.html) and [RFC-2348](https://www.rfc-editor.org/rfc/rfc2348.html)
    fn new_request(filename: &'a str, blocksize: Option<u16>, is_read: bool) -> Self {
        Self {
            is_read,
            filename,
            include_transfer_size: false,
            timeout_seconds: None,
            blocksize,
            unknown_options: &[],
        }
    }

    fn from_bytes_skip_opcode_check(data: &'a [u8], is_read: bool) -> TftpResult<Self> {
        let (filename, data) = printable_ascii_str_from_u8(&data[2..])?;
        let (mode, mut options_data) = printable_ascii_str_from_u8(data)?;
        let options_start = options_data;
        let mut blocksize = None;
        let mut include_transfer_size = false;
        let mut timeout_seconds = None;
        let mut has_unknown_options = false;
        while let Some((option, remainder)) = get_option_pair(options_data)? {
            if option.0.eq_ignore_ascii_case("blksize") {
                if blocksize.is_some() {
                    return Err(TftpError::OptionRepeated);
                }
                blocksize = Some(parse_blocksize(option.1)?)
            } else if option.0.eq_ignore_ascii_case("tsize") {
                if include_transfer_size {
                    return Err(TftpError::OptionRepeated);
                }
                if option.1 != "0" {
                    return Err(TftpError::BadFormatting);
                }
                include_transfer_size = true;
            } else if option.0.eq_ignore_ascii_case("timeout") {
                if timeout_seconds.is_some() {
                    return Err(TftpError::OptionRepeated);
                }
                let Ok(timeout) = option.1.parse() else {
                    return Err(TftpError::BadFormatting);
                };
                timeout_seconds = Some(timeout);
            } else {
                has_unknown_options = true;
            }
            options_data = remainder;
        }
        if !mode.eq_ignore_ascii_case("octet") {
            return Err(TftpError::BadFormatting);
        }
        Ok(Self {
            include_transfer_size,
            timeout_seconds,
            unknown_options: if has_unknown_options {
                options_start
            } else {
                &[]
            },
            blocksize,
            is_read,
            filename,
        })
    }

    /// returns true if this packet is a read request.
    pub fn is_read(&self) -> bool {
        self.is_read
    }
    /// returns true if this packet is a write request.
    pub fn is_write(&self) -> bool {
        !self.is_read()
    }

    fn opcode(&self) -> OpCode {
        if self.is_read() {
            OpCode::ReadRequest
        } else {
            OpCode::WriteRequest
        }
    }

    /// write this packet into the buffer `data`. The buffer is allowed to be larger than the packet size.
    /// Will return [TftpError::BufferTooSmall] if the packet doesn't fit but might still mutate the buffer.
    pub fn to_bytes(&self, buf: &'a mut [u8]) -> Result<usize, TftpError> {
        let mut write_target = BufferWriter::new(buf);
        write_target.push_bytes(&(self.opcode() as u16).to_be_bytes());
        write_target.push_bytes(self.filename.as_bytes());
        write_target.push_byte(0);
        write_target.push_bytes(b"octets\0");
        if let Some(blocksize) = self.blocksize {
            let _ = write!(write_target, "blksize\0{blocksize}\0");
        }
        if let Some(timeout) = self.timeout_seconds {
            let _ = write!(write_target, "timeout\0{timeout}\0");
        }
        if self.include_transfer_size {
            write_target.push_bytes(b"tsize\00\0");
        }
        if write_target.overflowed() {
            Err(TftpError::BufferTooSmall)
        } else {
            Ok(write_target.size)
        }
    }

    /// returns an iterator over all the options in this packet that this library does not know about.
    /// This iterator returns a result over tuple pairs of option names and values. Will return an error if either of these is not a null-terminated ascii string.
    /// see [RFC-2347](https://www.rfc-editor.org/rfc/inline-errata/rfc2347.html) for a definition of options.
    pub fn unknown_options(&self) -> impl Iterator<Item = TftpResult<(&str, &str)>> {
        OptionsIterator {
            buff: self.unknown_options,
        }
        .unknown()
    }
}

impl Ack {
    /// creates a new Ack packet with the given block number.
    pub fn new(block_nr: u16) -> Self {
        Self { block_nr }
    }

    fn from_bytes_skip_opcode_check(data: &[u8]) -> TftpResult<Self> {
        if data.len() < 4 {
            return Err(TftpError::BufferTooSmall);
        }
        let block_nr = u16::from_be_bytes([data[2], data[3]]);
        Ok(Self { block_nr })
    }

    /// write this packet into the provided buffer. On success returns the amounts of bytes written. If the buffer is too small to hold the packet, returns an [TftpError::BufferTooSmall] error.
    pub fn to_bytes(&self, buf: &mut [u8]) -> Result<usize, TftpError> {
        let n_bytes = 4;
        if buf.len() >= n_bytes {
            buf[0..2].copy_from_slice(&(OpCode::Error as u16).to_be_bytes());
            buf[2..4].copy_from_slice(&self.block_nr.to_be_bytes());
            Ok(n_bytes)
        } else {
            Err(TftpError::BufferTooSmall)
        }
    }
}

impl<'a> Error<'a> {
    /// creates a new error packtet with the given [ErrorCode] and message.
    pub fn new(error_code: ErrorCode, message: &'a str) -> Self {
        Self {
            error_code,
            message,
        }
    }
    fn from_bytes_skip_opcode_check(data: &'a [u8]) -> TftpResult<Self> {
        if data.len() < 4 {
            return Err(TftpError::BufferTooSmall);
        }
        let error_code = ErrorCode::possibly_invalid(u16::from_be_bytes([data[2], data[3]]));
        Ok(Self {
            error_code,
            message: printable_ascii_str_from_u8(&data[4..])?.0,
        })
    }
    /// write this packet into the provided buffer. On success returns the amounts of bytes written. If the buffer is too small to hold the packet, returns an [TftpError::BufferTooSmall] error.
    pub fn to_bytes(&self, buf: &'a mut [u8]) -> Result<usize, TftpError> {
        let n_bytes = 4 + self.message.len() + 1;
        if n_bytes > buf.len() {
            return Err(TftpError::BufferTooSmall);
        }
        buf[0..2].copy_from_slice(&(OpCode::Error as u16).to_be_bytes());
        buf[2..4].copy_from_slice(&self.error_code.0.to_be_bytes());
        buf[4..4 + self.message.bytes().len()].copy_from_slice(self.message.as_bytes());
        buf[4 + self.message.bytes().len()] = 0;
        Ok(n_bytes)
    }
}

impl OptionAck<'static> {
    /// Creates an Option Ack packet, optionally including a blocksize as defined in [RFC-2348](https://datatracker.ietf.org/doc/html/rfc2348), transfer size([RFC-2349](https://www.rfc-editor.org/rfc/rfc2349.html)), or timeout ([RFC-2349](https://www.rfc-editor.org/rfc/rfc2349.html)).
    pub fn new(
        blocksize: Option<u16>,
        transfer_size: Option<u64>,
        timeout_seconds: Option<NonZeroU8>,
    ) -> Self {
        //can't _construct_ an option ack with unknown fields because the server wouldn't know how to handle them.
        // we don't support timeouts in the server either, so we don't construct those either.
        Self {
            blocksize,
            transfer_size,
            timeout_seconds,
            unknown_options: &[],
        }
    }
}

impl<'a> OptionAck<'a> {
    fn from_bytes_skip_opcode_check(data: &'a [u8]) -> TftpResult<Self> {
        let mut data = &data[2..];
        let mut blocksize = None;
        let mut transfer_size = None;
        let mut timeout_seconds = None;
        let original_options = data;
        let mut has_unknown_options = false;
        while let Some((option, remainder)) = get_option_pair(data)? {
            if option.0.eq_ignore_ascii_case("blksize") {
                if blocksize.is_some() {
                    return Err(TftpError::OptionRepeated);
                }
                blocksize = Some(parse_blocksize(option.1)?)
            } else if option.0.eq_ignore_ascii_case("tsize") {
                if transfer_size.is_some() {
                    return Err(TftpError::OptionRepeated);
                }
                let Ok(transfer_size_val) = option.1.parse() else {
                    return Err(TftpError::BadFormatting);
                };
                transfer_size = Some(transfer_size_val);
            } else if option.0.eq_ignore_ascii_case("timeout") {
                if timeout_seconds.is_some() {
                    return Err(TftpError::OptionRepeated);
                }
                let Ok(timeout) = option.1.parse() else {
                    return Err(TftpError::BadFormatting);
                };
                timeout_seconds = Some(timeout);
            } else {
                has_unknown_options = true;
            }
            data = remainder;
        }
        Ok(Self {
            blocksize,
            transfer_size,
            timeout_seconds,
            unknown_options: if has_unknown_options {
                original_options
            } else {
                &[]
            },
        })
    }

    /// write this packet into the buffer `data`. The buffer is allowed to be larger than the packet size.
    /// Will return [TftpError::BufferTooSmall] if the packet doesn't fit but might still mutate the buffer.
    pub fn to_bytes(&self, buf: &'a mut [u8]) -> Result<usize, TftpError> {
        let mut write_target = BufferWriter::new(buf);
        write_target.push_bytes(&(OpCode::OptionAck as u16).to_be_bytes());
        if let Some(blocksize) = self.blocksize {
            let _ = write!(write_target, "blksize\0{blocksize}\0");
        }
        if let Some(tsize) = self.transfer_size {
            let _ = write!(write_target, "tsize\0{tsize}\0");
        }
        if let Some(timeout) = self.timeout_seconds {
            let _ = write!(write_target, "timeout\0{timeout}\0");
        }
        if write_target.overflowed() {
            Err(TftpError::BufferTooSmall)
        } else {
            Ok(write_target.size)
        }
    }

    ///returns true if this packet has any options set.
    pub fn is_empty(&self) -> bool {
        self.blocksize.is_none()
            && self.timeout_seconds.is_none()
            && self.transfer_size.is_none()
            && self.unknown_options.is_empty()
    }

    /// returns an iterator over all the options in this packet that this library does not know about.
    /// This iterator returns a result over tuple pairs of option names and values. Will return an error if either of these is not a null-terminated ascii string.
    /// see [RFC-2347](https://www.rfc-editor.org/rfc/inline-errata/rfc2347.html) for a definition of options.
    pub fn unknown_options(&self) -> impl Iterator<Item = TftpResult<(&str, &str)>> {
        OptionsIterator {
            buff: self.unknown_options,
        }
        .unknown()
    }
}

/// an iterator over name-value pairs of options in a read/write-request packet or option-acknowledge packet
pub struct OptionsIterator<'a> {
    buff: &'a [u8],
}

impl<'a> OptionsIterator<'a> {
    /// iterate only over the options that are not understood by this crate (i.e. anything but `blksize`, `timeout` and `tsize`).
    pub fn unknown(self) -> impl Iterator<Item = TftpResult<(&'a str, &'a str)>> {
        self.into_iter().filter(|x| match x {
            Ok((name, _)) => match *name {
                "blksize" | "timeout" | "tsize" => false,
                _ => true,
            },
            Err(_) => true,
        })
    }
}

impl<'a> Iterator for OptionsIterator<'a> {
    type Item = TftpResult<(&'a str, &'a str)>;
    fn next(&mut self) -> Option<Self::Item> {
        match get_option_pair(self.buff) {
            Ok(Some((pair, remainder))) => {
                self.buff = remainder;
                Some(Ok(pair))
            }
            Err(e) => Some(Err(e)),
            Ok(None) => None,
        }
    }
}
