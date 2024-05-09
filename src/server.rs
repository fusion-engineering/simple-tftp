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

/// A TFTP Server implementation
pub struct Server {
    sock: TFTPSocket,
}

impl Server {
    /// creates a new server bound to ip address `ip` and port 69.
    pub fn connect(ip: IpAddr) -> IoResult<Self> {
        Self::connect_with_port(ip, 69)
    }

    /// creates a new server bound to ip address `ip` and port `port`.
    pub fn connect_with_port(ip: IpAddr, port: u16) -> IoResult<Self> {
        Ok(Self {
            sock: TFTPSocket::new(SocketAddr::new(ip, port), None)?,
        })
    }

    /// sets the read timeout of the underlying socket. Note that this has nothing to do with the timeout option described in [RFC-2349](https://www.rfc-editor.org/rfc/rfc2349.html).
    pub fn set_read_timeout(&mut self, timeout: Option<Duration>) -> IoResult<()> {
        self.sock.sock.set_read_timeout(timeout)
    }

    /// sets the write timeout of the underlying socket. Note that this has nothing to do with the timeout option described in [RFC-2349](https://www.rfc-editor.org/rfc/rfc2349.html).
    pub fn set_write_timeout(&mut self, timeout: Option<Duration>) -> IoResult<()> {
        self.sock.sock.set_write_timeout(timeout)
    }

    /// gets the next request from a client and returns it plus the adress of the client.
    /// wil return an error if the next packet received is not a request.
    pub fn get_next_request_from(&mut self) -> IoResult<(Request<'_>, SocketAddr)> {
        match self.sock.get_next_message_from()? {
            (Packet::Request(req), addr) => Ok((req, addr)),
            _ => {
                return Err(IoError::new(
                    std::io::ErrorKind::InvalidData,
                    "Invalid packet received",
                ))
            }
        }
    }

    /// transfers the data contained in `source` to `target`, optionally using the TFTP extensions described in `options`.
    pub fn create_transfer_to<R: std::io::Read>(
        &self,
        target: SocketAddr,
        source: R,
        options: OptionAck<'static>,
    ) -> IoResult<Transfer<R>> {
        Transfer::new(source, self.sock.sock.local_addr()?.ip(), target, options)
    }

    /// sends the error message `error` to the client at `addr`.
    pub fn send_error_to(&mut self, error: Error, addr: SocketAddr) -> IoResult<()> {
        self.sock.send_message_to(Packet::Error(error), addr)
    }

    /// return the ip this socket is bound to.
    pub fn ip(&self) -> Result<IpAddr, IoError> {
        self.sock.sock.local_addr().map(|a| a.ip())
    }
}

/// An in progress transfer between a server and a client
/// does nothing until it is consumed with the [`finish`](Transfer::finish) method

pub struct Transfer<R: Read> {
    sock: TFTPSocket,
    source: DataStream<R>,
    options: OptionAck<'static>,
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

    /// executes the transfer.
    ///
    ///an error can occur for 4 reasons:
    /// 1. we have hit an io-error reading the file,
    /// 2. we hit an io-error while doing udp transfers
    /// 3. or the client has send us an error packet during the transfer,
    /// 4. or the client has send us an invalid reply.
    ///
    /// in the case of 1, this function will automatically try to send an error packet to the client
    /// before returning the initial IO error.
    /// in all other cases it will not notify the client. As either the client Explicitly errored out, or the client messed up
    /// or we're having issues with the underlying UDP and will likely fail sending the error message too.
    pub fn finish(mut self) -> Result<(), IoError> {
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
