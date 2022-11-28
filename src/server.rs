use crate::{
    packet::{Ack, DataStream, Error, Packet, Request},
    TFTPSocket,
};
use std::{
    io::{Error as IoError, Read, Result as IoResult},
    net::{IpAddr, SocketAddr},
    result::Result,
};

pub struct Server {
    sock: TFTPSocket,
}

impl Server {
    pub fn connect(ip: IpAddr) -> IoResult<Self> {
        Self::connect_with_port(ip, 69)
    }

    pub fn connect_with_port(ip: IpAddr, port: u16) -> IoResult<Self> {
        Ok(Self {
            sock: TFTPSocket::new(SocketAddr::new(ip, port), None)?,
        })
    }

    pub fn get_next_request_from(&mut self) -> (Request<'_>, SocketAddr) {
        match self.sock.get_next_message_from().unwrap() {
            (Packet::Request(req), addr) => (req, addr),
            _ => panic!("invalid packet received"),
        }
    }

    pub fn create_transfer_to<R: std::io::Read>(
        &self,
        target: SocketAddr,
        source: R,
    ) -> Result<Transfer<R>, IoError> {
        Transfer::new_with_blocksize(source, self.sock.sock.local_addr()?.ip(), target, 512)
    }

    pub fn send_error_to(&mut self, error: Error, addr: SocketAddr) -> Result<(), IoError> {
        self.sock.send_message_to(Packet::Error(error), addr)
    }
}

pub struct Transfer<R: Read> {
    sock: TFTPSocket,
    source: DataStream<R>,
}

impl<R: Read> Transfer<R> {
    fn new_with_blocksize(
        source: R,
        ip: IpAddr,
        target: SocketAddr,
        block_size: u16,
    ) -> Result<Self, IoError> {
        Ok(Self {
            sock: TFTPSocket::new(SocketAddr::new(ip, 0), Some(target))?,
            source: DataStream::new(source, block_size),
        })
    }

    pub fn finish(mut self) -> Result<(), IoError> {
        while let Some(bytes) = self.source.next_raw()? {
            self.sock.sock.send(bytes)?;
            let (reply, _) = self.sock.get_next_message_from()?;
            let current_block = self.source.last_block();
            match reply {
                Packet::Ack(Ack { block_nr: block }) if block == current_block => {}
                Packet::Error(e) => {
                    return Err(IoError::new(
                        std::io::ErrorKind::Other,
                        format!(
                            "Received TFTP error ({} : \"{}\") while waiting on ({current_block})",
                            e.error_code, e.message
                        ),
                    ));
                }
                e => {
                    return Err(IoError::new(
                        std::io::ErrorKind::InvalidData,
                        format!(
                        "Received unexpected packet while waiting on Ack({current_block}): {e:?}"
                    ),
                    ));
                }
            }
        }
        Ok(())
    }
}
