use crate::packet::{Data, OpCode};

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
