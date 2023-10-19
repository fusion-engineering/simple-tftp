use crate::error::{Error as TftpError, Result as TftpResult};
#[derive(Debug, Eq, PartialEq)]
#[repr(u16)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ErrorCode(u16);
impl ErrorCode {
    pub const NOT_DEFINED: Self = Self(0);
    pub const FILE_NOT_FOUND: Self = Self(1);
    pub const ACCESS_VIOLATION: Self = Self(2);
    pub const DISK_FULL_OR_ALLOCATION_EXCEEDED: Self = Self(3);
    pub const ILLEGAL_TFTP_OPERATION: Self = Self(4);
    pub const UNKNOWN_TRANSFER_ID: Self = Self(5);
    pub const FILE_ALREADY_EXISTS: Self = Self(6);
    pub const NO_SUCH_USER: Self = Self(7);
    fn possibly_invalid(code: u16) -> Self {
        Self(code)
    }

    pub fn is_valid(&self) -> bool {
        self.0 <= 7
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

#[derive(Debug)]
pub struct Request<'a> {
    is_read: bool,
    pub filename: &'a str,
    //only the octet mode is supported so it isn't stored here
    pub blocksize: Option<u16>,
    pub include_transfer_size: bool,
    pub timeout_seconds: Option<u32>,
    //from RFC2347, requests can be at most 512 octets.
    //they must contain a null-terminated name and value, so we divide by 4.
    // to get an upper bound on the amount of entries
    unknown_options: arrayvec::ArrayVec<(&'a str, &'a str), { 512 / 4 }>,
}

//want to use a DST here but transmuting between bytes and DST-fat pointers is undefined.
// this representation is great for reading, but not for writting as it can't guarentee the data is contigous
// splitting into two is cumbersome.
#[derive(Debug)]
pub struct Data<'a> {
    block_nr: u16,
    data: &'a [u8],
}

#[derive(Debug)]
pub struct Ack {
    pub block_nr: u16,
}

#[derive(Debug)]
pub struct Error<'a> {
    pub error_code: ErrorCode,
    pub message: &'a str,
}

#[derive(Debug)]
pub struct OptionAck<'a> {
    pub blocksize: Option<u16>,
    pub transfer_size: Option<u64>,
    pub timeout_seconds: Option<u32>,
    //from RFC2347, requests can be at most 512 octets.
    //they must contain a null-terminated name and value, so we divide by 4.
    // to get an upper bound on the amount of entries
    unknown_options: arrayvec::ArrayVec<(&'a str, &'a str), { 512 / 4 }>,
}

#[derive(Debug)]
pub enum Packet<'a> {
    Data(Data<'a>),
    Request(Request<'a>),
    Error(Error<'a>),
    Ack(Ack),
    OptionAck(OptionAck<'a>),
}

impl<'a> Packet<'a> {
    pub fn new_data(block_nr: u16, data: &'a [u8]) -> Self {
        Self::Data(Data::new(block_nr, data))
    }

    pub fn new_read_request(filename: &'a str, blocksize: Option<u16>) -> Self {
        Self::Request(Request::new_read_request(filename, blocksize))
    }

    pub fn new_write_request(filename: &'a str, blocksize: Option<u16>) -> Self {
        Self::Request(Request::new_write_request(filename, blocksize))
    }

    pub fn new_error(error_code: ErrorCode, message: &'a str) -> Self {
        Self::Error(Error::new(error_code, message))
    }

    pub fn new_ack(block_nr: u16) -> Self {
        Self::Ack(Ack::new(block_nr))
    }

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
                OpCode::Data => Self::Data(Data::from_bytes_skip_opcode_check(data)),
                OpCode::Acknowledgement => Self::Ack(Ack::from_bytes_skip_opcode_check(data)),
                OpCode::Error => Self::Error(Error::from_bytes_skip_opcode_check(data)),
                OpCode::OptionAck => Self::OptionAck(OptionAck::from_bytes_skip_opcode_check(data)),
            })
        })?
    }

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

struct ChunkyReader<R: std::io::Read> {
    inner: R,
}

impl<R: std::io::Read> ChunkyReader<R> {
    pub fn new(inner: R) -> Self {
        Self { inner }
    }
    pub fn try_read_exact(
        &mut self,
        mut buf: &mut [u8],
    ) -> std::result::Result<usize, std::io::Error> {
        let mut bytes_read = 0;
        while !buf.is_empty() {
            match self.inner.read(buf) {
                Ok(0) => return Ok(bytes_read),
                Ok(n) => {
                    let tmp = buf;
                    bytes_read += n;
                    buf = &mut tmp[n..];
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::Interrupted => {}
                Err(e) => return Err(e),
            }
        }
        Ok(bytes_read)
    }
}

pub struct DataStream<R: std::io::Read> {
    source: ChunkyReader<R>,
    block_counter: u16,
    is_finished: bool,
    buffer: Vec<u8>,
}

impl<'a, R: std::io::Read> DataStream<R> {
    pub fn new(source: R, blocksize: u16) -> Self {
        let mut buffer = vec![0u8; 4 + blocksize as usize];
        buffer[0..2].copy_from_slice(&(OpCode::Data as u16).to_be_bytes());
        Self {
            source: ChunkyReader::new(source),
            is_finished: false,
            block_counter: 0,
            buffer,
        }
    }
    pub fn blocksize(&self) -> usize {
        self.buffer.len() - 4
    }
    pub fn next_raw(&mut self) -> std::io::Result<Option<&[u8]>> {
        if self.is_finished {
            return Ok(None);
        }
        self.block_counter = self.block_counter.wrapping_add(1);
        self.buffer[2..4].copy_from_slice(&self.block_counter.to_be_bytes());
        match self.source.try_read_exact(&mut self.buffer[4..]) {
            Ok(bytes_read) => {
                if bytes_read < self.blocksize() {
                    self.is_finished = true;
                }
                Ok(Some(&self.buffer[0..4 + bytes_read]))
            }
            Err(e) => Err(e),
        }
    }

    pub fn next_packet(&mut self) -> std::io::Result<Option<Data<'_>>> {
        self.next_raw().map(|opt| opt.map(Data::from_bytes))
    }

    pub fn last_block(&self) -> u16 {
        self.block_counter
    }
}

impl<'a> Data<'a> {
    pub fn new(block_nr: u16, data: &'a [u8]) -> Self {
        Self { block_nr, data }
    }

    pub fn from_bytes(data: &'a [u8]) -> Self {
        let opcode = u16::from_be_bytes([data[0], data[1]]);
        assert_eq!(opcode, OpCode::Data as u16);
        Self::from_bytes_skip_opcode_check(data)
    }

    fn from_bytes_skip_opcode_check(data: &'a [u8]) -> Self {
        let block_nr = u16::from_be_bytes([data[2], data[3]]);
        let data = &data[4..];
        Self { block_nr, data }
    }

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
// todo: validate netascii?
// The data is supposed to be netascii, a really outdated format. no good reason to limit ourselves to it aside from "the standard says so"
// and this function will usually be called on data generated by a remote host, which may not be compliant itself
// and instead send utf-8 or 'normal' ascii.
fn printable_ascii_str_from_u8(data: &[u8]) -> (&str, &[u8]) {
    let first_non_ascii = data.into_iter().position(|&n| n < 32 || n > 127);
    if let Some(index) = first_non_ascii {
        if data[index] == 0 {
            return unsafe {
                (
                    std::str::from_utf8_unchecked(&data[..index]),
                    &data[(index + 1).min(data.len())..],
                )
            };
        }
    }
    //todo: bubble error instead of panic.
    panic!("invalid data, does not contain a null-terminated ascii string");
}

fn get_option_pair(data: &[u8]) -> Option<((&str, &str), &[u8])> {
    if data.len() == 0 {
        None
    } else {
        let (name, data) = printable_ascii_str_from_u8(data);
        let (value, data) = printable_ascii_str_from_u8(data);
        Some(((name, value), data))
    }
}

fn parse_blocksize(as_str: &str) -> u16 {
    let requested_blocksize = as_str
        .parse::<u16>()
        .expect("couldn't parse blksize option as u16");
    //Valid values range between "8" and "65464" octets, inclusive.
    if requested_blocksize < 8 || requested_blocksize > 65464 {
        panic!(
            "requested blocksize {requested_blocksize} falls outside of the valid-range 8..=65464"
        );
    } else {
        requested_blocksize
    }
}

impl<'a> Request<'a> {
    pub fn new_read_request(filename: &'a str, blocksize: Option<u16>) -> Self {
        Self::new_request(filename, blocksize, true)
    }

    pub fn new_write_request(filename: &'a str, blocksize: Option<u16>) -> Self {
        Self::new_request(filename, blocksize, false)
    }

    fn new_request(filename: &'a str, blocksize: Option<u16>, is_read: bool) -> Self {
        Self {
            is_read,
            filename,
            include_transfer_size: false,
            timeout_seconds: None,
            blocksize,
            unknown_options: arrayvec::ArrayVec::new(),
        }
    }

    pub fn from_bytes(data: &'a [u8]) -> TftpResult<Self> {
        let opcode = u16::from_be_bytes([data[0], data[1]]);
        assert!(opcode == OpCode::ReadRequest as u16 || opcode == OpCode::WriteRequest as u16);
        let is_read = if opcode == OpCode::ReadRequest as u16 {
            true
        } else {
            false
        };
        Self::from_bytes_skip_opcode_check(data, is_read)
    }

    fn from_bytes_skip_opcode_check(data: &'a [u8], is_read: bool) -> TftpResult<Self> {
        let (filename, data) = printable_ascii_str_from_u8(&data[2..]);
        let (mode, mut options_data) = printable_ascii_str_from_u8(data);
        let mut blocksize = None;
        let mut include_transfer_size = false;
        let mut timeout_seconds = None;
        let mut vec = arrayvec::ArrayVec::new();
        while let Some((option, remainder)) = get_option_pair(options_data) {
            if option.0.eq_ignore_ascii_case("blksize") {
                if blocksize.is_some() {
                    panic!("blksize option specified multiple times in request!");
                }
                blocksize = Some(parse_blocksize(option.1))
            } else if option.0.eq_ignore_ascii_case("tsize") {
                if include_transfer_size {
                    panic!("tsize option specified multiple times in request!");
                }
                assert_eq!(option.1, "0");
                include_transfer_size = true;
            } else if option.0.eq_ignore_ascii_case("timeout") {
                if timeout_seconds.is_some() {
                    panic!("timeout option specified multiple times in request!");
                }
                let timeout = option.1.parse::<u32>().expect("failed to parse time-out");
                timeout_seconds = Some(timeout);
            } else if let Err(_) = vec.try_push(option) {
                return Err(TftpError::BufferTooSmall);
            }
            options_data = remainder;
        }
        assert!(mode.eq_ignore_ascii_case("octet"));
        Ok(Self {
            include_transfer_size,
            timeout_seconds,
            unknown_options: vec,
            blocksize,
            is_read,
            filename,
        })
    }

    pub fn is_read(&self) -> bool {
        self.is_read
    }
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

    pub fn to_bytes(&self, buf: &'a mut [u8]) -> Result<usize, TftpError> {
        let mode = b"octets\0";
        let blksize = b"blksize\0";
        let tsize = b"tsize\0";
        let timeout = b"timeout\0";
        let blocksize_val = self.blocksize.map(|u| {
            let mut formated = format!("{u}").into_bytes();
            formated.push(0);
            formated
        });
        let blocksize_n_bytes = if let Some(blocksize) = &blocksize_val {
            blocksize.len() + blksize.len()
        } else {
            0
        };
        let timeout_val = self.timeout_seconds.map(|u| {
            let mut formated = format!("{u}").into_bytes();
            formated.push(0);
            formated
        });
        let timeout_n_bytes = if let Some(timeout_val) = &timeout_val {
            timeout_val.len() + timeout.len()
        } else {
            0
        };
        let tsize_n_bytes = if self.include_transfer_size {
            tsize.len() + 2
        } else {
            0
        };
        let name_len = self.filename.len();
        let mode_len = mode.len();
        let n_bytes =
            2 + name_len + 1 + mode_len + blocksize_n_bytes + tsize_n_bytes + timeout_n_bytes;

        if n_bytes > buf.len() {
            return Err(TftpError::BufferTooSmall);
        }
        let opcode = (self.opcode() as u16).to_be_bytes();
        buf[0..2].copy_from_slice(&opcode);
        buf[2..2 + name_len].copy_from_slice(self.filename.as_bytes());
        buf[2 + name_len] = 0;
        buf[2 + name_len + 1..2 + name_len + 1 + mode_len].copy_from_slice(mode);
        let offset = 2 + name_len + 1 + mode_len;
        let offset = if let Some(blocksize) = blocksize_val {
            buf[offset..offset + blksize.len()].copy_from_slice(blksize);
            buf[offset + blksize.len()..offset + blksize.len() + blocksize.len()]
                .copy_from_slice(&blocksize);
            offset + blksize.len() + blocksize.len()
        } else {
            offset
        };
        let offset = if let Some(timeout_val) = timeout_val {
            buf[offset..offset + timeout.len()].copy_from_slice(timeout);
            buf[offset + timeout.len()..offset + timeout.len() + timeout_val.len()]
                .copy_from_slice(&timeout_val);
            timeout_val.len() + timeout.len()
        } else {
            offset
        };
        if self.include_transfer_size {
            buf[offset..offset + tsize.len()].copy_from_slice(tsize);
            buf[offset + tsize.len()..offset + tsize.len() + 2].copy_from_slice(b"0\0");
        }
        Ok(n_bytes)
    }

    pub fn unknown_options(&self) -> &[(&str, &str)] {
        &self.unknown_options
    }
}

impl Ack {
    pub fn new(block_nr: u16) -> Self {
        Self { block_nr }
    }
    pub fn from_bytes(data: &[u8]) -> Self {
        let opcode = u16::from_be_bytes([data[0], data[1]]);
        assert_eq!(opcode, OpCode::Acknowledgement as u16);
        Self::from_bytes_skip_opcode_check(data)
    }

    fn from_bytes_skip_opcode_check(data: &[u8]) -> Self {
        //todo: bounds checking
        let block_nr = u16::from_be_bytes([data[2], data[3]]);
        Self { block_nr }
    }

    pub fn to_bytes(&self, buf: &mut [u8]) -> Result<usize, TftpError> {
        let n_bytes = 4;
        assert!(buf.len() >= n_bytes);
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
    pub fn new(error_code: ErrorCode, message: &'a str) -> Self {
        Self {
            error_code,
            message,
        }
    }

    pub fn from_bytes(data: &'a [u8]) -> Self {
        let opcode = u16::from_be_bytes([data[0], data[1]]);
        assert_eq!(opcode, OpCode::Error as u16);
        Self::from_bytes_skip_opcode_check(data)
    }

    fn from_bytes_skip_opcode_check(data: &'a [u8]) -> Self {
        let error_code = ErrorCode::possibly_invalid(u16::from_be_bytes([data[2], data[3]]));
        Self {
            error_code,
            message: printable_ascii_str_from_u8(&data[4..]).0,
        }
    }
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

impl<'a> OptionAck<'a> {
    pub fn new(blocksize: Option<u16>, transfer_size: Option<u64>) -> Self {
        //can't _construct_ an option ack with unknown fields because the server wouldn't know how to handle them.
        // we don't support timeouts in the server either, so we don't construct those either.
        Self {
            blocksize,
            transfer_size,
            timeout_seconds: None,
            unknown_options: arrayvec::ArrayVec::new(),
        }
    }
    pub fn from_bytes(data: &'a [u8]) -> Self {
        let opcode = u16::from_be_bytes([data[0], data[1]]);
        assert_eq!(opcode, OpCode::OptionAck as u16);
        Self::from_bytes_skip_opcode_check(data)
    }
    fn from_bytes_skip_opcode_check(data: &'a [u8]) -> Self {
        let mut data = &data[2..];
        let mut vec = arrayvec::ArrayVec::new();
        let mut blocksize = None;
        let mut transfer_size = None;
        let mut timeout_seconds = None;
        while let Some((option, remainder)) = get_option_pair(data) {
            if option.0.eq_ignore_ascii_case("blksize") {
                if blocksize.is_some() {
                    panic!("blksize option specified multiple times in request!");
                }
                blocksize = Some(parse_blocksize(option.1))
            } else if option.0.eq_ignore_ascii_case("tsize") {
                if transfer_size.is_some() {
                    panic!("tsize option specified multiple times in request!");
                }
                let transfer_size_val = option.1.parse().expect("failed to parse transfer-size");
                transfer_size = Some(transfer_size_val);
            } else if option.0.eq_ignore_ascii_case("timeout") {
                if timeout_seconds.is_some() {
                    panic!("timeout option specified multiple times in request!");
                }
                let timeout = option.1.parse().expect("failed to parse time-out");
                timeout_seconds = Some(timeout);
            } else {
                vec.push(option);
            }
            data = remainder;
        }
        Self {
            blocksize,
            transfer_size,
            timeout_seconds,
            unknown_options: vec,
        }
    }

    pub fn to_bytes(&self, buf: &'a mut [u8]) -> Result<usize, TftpError> {
        let blksize = b"blksize\0";
        let blocksize_val = self.blocksize.map(|u| {
            let mut formated = format!("{u}").into_bytes();
            formated.push(0);
            formated
        });
        let blocksize_n_bytes = if let Some(blocksize) = &blocksize_val {
            blocksize.len() + blksize.len()
        } else {
            0
        };
        let n_bytes = 2 + blocksize_n_bytes; //we ignore unknown options here as we should never send options we don't understand yet.
        if n_bytes > buf.len() {
            return Err(TftpError::BufferTooSmall);
        }
        buf[0..2].copy_from_slice(&(OpCode::OptionAck as u16).to_be_bytes());
        if let Some(blocksize) = blocksize_val {
            buf[2..2 + blksize.len()].copy_from_slice(blksize);
            buf[2 + blksize.len()..2 + blksize.len() + blocksize.len()].copy_from_slice(&blocksize);
        }
        Ok(n_bytes)
    }

    pub fn is_empty(&self) -> bool {
        self.blocksize.is_none()
            && self.timeout_seconds.is_none()
            && self.transfer_size.is_none()
            && self.unknown_options.is_empty()
    }

    pub fn unknown_options(&self) -> &[(&str, &str)] {
        &self.unknown_options
    }
}
