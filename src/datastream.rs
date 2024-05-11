use crate::packet::OpCode;

struct ChunkyReader<R: std::io::Read> {
    inner: R,
}

impl<R: std::io::Read> ChunkyReader<R> {
    pub fn new(inner: R) -> Self {
        Self { inner }
    }
    /// similar to https://doc.rust-lang.org/std/io/trait.Read.html#method.read_exact
    /// but will will return Ok(bytes_read) when it encounters an EOF before filling the buffer
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
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {}
                Err(e) => return Err(e),
            }
        }
        Ok(bytes_read)
    }
}

/// Wrapper around a source that implements [std::io::Read] that can be used to read out fixed size chunks at a time.
/// similar to the chunks method on slices. This struct serves as a helper for splitting a stream like source into packets.
pub(crate) struct DataStream<R: std::io::Read> {
    source: ChunkyReader<R>,
    block_counter: u16,
    is_finished: bool,
    buffer: Vec<u8>,
}

impl<'a, R: std::io::Read> DataStream<R> {
    /// creates a new DataStream that will split the source up into chunks of blocksize bytes.
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

    /// returns the blocksize of this DataStream
    pub fn blocksize(&self) -> usize {
        self.buffer.len() - 4
    }

    pub(crate) fn next_raw(&mut self) -> std::io::Result<Option<&[u8]>> {
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
            Err(e) => {
                self.is_finished = true;
                Err(e)
            }
        }
    }

    pub fn last_block(&self) -> u16 {
        self.block_counter
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;
    #[test]
    fn reader() {
        let source = b"aaaabbbbccccddddeeeexxx";
        let mut buff = [0; 4];
        let mut reader = ChunkyReader::new(&source[..]);
        let n = reader.try_read_exact(&mut buff).unwrap();
        assert_eq!(&buff[..n], "aaaa".as_bytes());
        let n = reader.try_read_exact(&mut buff).unwrap();
        assert_eq!(&buff[..n], "bbbb".as_bytes());
        let n = reader.try_read_exact(&mut buff).unwrap();
        assert_eq!(&buff[..n], "cccc".as_bytes());
        let n = reader.try_read_exact(&mut buff).unwrap();
        assert_eq!(&buff[..n], "dddd".as_bytes());
        let n = reader.try_read_exact(&mut buff).unwrap();
        assert_eq!(&buff[..n], "eeee".as_bytes());
        let n = reader.try_read_exact(&mut buff).unwrap();
        assert_eq!(&buff[..n], "xxx".as_bytes());
    }

    #[test]
    fn datastream_blocksize() {
        let source = b"aaaabbbbccccddddeeeexxx";
        for bs in &[0, 3, 4, 7, 999, u16::MAX] {
            let ds = DataStream::new(&source[..], *bs);
            assert_eq!(ds.blocksize(), *bs as usize)
        }
    }

    //todo: add tests for the error cases
    // e.g. implement a reader that fails after a few bytes and check that it doesn't return garbage
}
