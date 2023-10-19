use crate::{
    datastream::DataStream,
    packet::{Ack, Error, OptionAck, Packet, Request},
    socket::TFTPSocket,
};
use std::{
    io::{Error as IoError, Read, Result as IoResult},
    net::{IpAddr, SocketAddr},
    time::Duration,
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

    pub fn set_read_timeout(&mut self, timeout: Option<Duration>) -> IoResult<()> {
        self.sock.sock.set_read_timeout(timeout)
    }

    pub fn set_write_timeout(&mut self, timeout: Option<Duration>) -> IoResult<()> {
        self.sock.sock.set_write_timeout(timeout)
    }

    pub fn get_next_request_from(&mut self) -> IoResult<(Request<'_>, SocketAddr)> {
        match self.sock.get_next_message_from()? {
            (Packet::Request(req), addr) => Ok((req, addr)),
            //todo: don't panic here
            _ => panic!("invalid packet received"),
        }
    }

    pub fn create_transfer_to<R: std::io::Read>(
        &self,
        target: SocketAddr,
        source: R,
        options: OptionAck<'static>,
    ) -> IoResult<Transfer<R>> {
        Transfer::new(source, self.sock.sock.local_addr()?.ip(), target, options)
    }

    pub fn send_error_to(&mut self, error: Error, addr: SocketAddr) -> IoResult<()> {
        self.sock.send_message_to(Packet::Error(error), addr)
    }

    pub fn ip(&self) -> Result<IpAddr, IoError> {
        self.sock.sock.local_addr().map(|a| a.ip())
    }
}

pub struct Transfer<R: Read> {
    sock: TFTPSocket,
    source: DataStream<R>,
    options: OptionAck<'static>,
}

pub fn do_transfer_with_options<R: Read>(
    source: R,
    ip: IpAddr,
    target: SocketAddr,
    options: crate::packet::OptionAck<'static>,
) -> Result<(), IoError> {
    Transfer::new(source, ip, target, options)?.finish()
}

impl<R: Read> Transfer<R> {
    fn new(
        source: R,
        ip: IpAddr,
        target: SocketAddr,
        options: OptionAck<'static>,
    ) -> IoResult<Self> {
        Ok(Self {
            sock: TFTPSocket::new(SocketAddr::new(ip, 0), Some(target))?,
            source: DataStream::new(source, options.blocksize.unwrap_or(512)),
            options,
        })
    }

    // checks that `reply` is an ACK packet with block_nr `current_block`
    fn check_ack(reply: Packet, current_block: u16) -> IoResult<()> {
        match reply {
            Packet::Ack(Ack { block_nr: block }) if block == current_block => Ok(()),
            Packet::Error(e) => Err(IoError::new(
                std::io::ErrorKind::Other,
                format!(
                    "Received TFTP error ({} : \"{}\") while waiting on ({current_block})",
                    e.error_code, e.message
                ),
            )),
            e => Err(IoError::new(
                std::io::ErrorKind::InvalidData,
                format!("Received unexpected packet while waiting on Ack({current_block}): {e:?}"),
            )),
        }
    }
    pub fn finish(mut self) -> Result<(), IoError> {
        // let mut sock = TFTPSocket::new(SocketAddr::new(ip, 0), Some(target))?;
        // let mut source = DataStream::new(source, options.blocksize.unwrap_or(512));
        if !self.options.is_empty() {
            self.sock.send_message(Packet::OptionAck(self.options))?;
            let (reply, _) = self.sock.get_next_message_from()?;
            Self::check_ack(reply, 0)?;
        }
        while let Some(bytes) = {
            match self.source.next_raw() {
                Ok(x) => x,
                //if source.next_raw() fails to get bytes, i.e. calling "read" on the underlying source fails,
                // try to notify the client of the error before returning
                Err(e) => {
                    let _may_fail = self.sock.send_message(Packet::new_error(
                        crate::packet::ErrorCode::NOT_DEFINED,
                        "Unexpected IO error",
                    ));
                    return Err(e);
                }
            }
        } {
            self.sock.sock.send(bytes)?;
            let (reply, _) = self.sock.get_next_message_from()?;
            Self::check_ack(reply, self.source.last_block())?;
        }
        Ok(())
    }
}
