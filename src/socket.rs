use crate::Packet;
use std::{
    io::{Error as IoError, Result as IoResult},
    net::{SocketAddr, UdpSocket},
};

/// Wraps a UDP socket + buffer and exposes methods common to both server and client for reading and sending messages.
/// unless you're implementing your own server or client, you probably want to use the [`Server`](crate::server::Server) struct instead.
pub struct TFTPSocket {
    pub(crate) sock: UdpSocket,
    buffer: Vec<u8>,
}

impl TFTPSocket {
    /// creates a new UDP socket bound to `bind_addr` and optionally connects it to `connect_addr`.
    /// note that the default port for TFTP is 69.
    pub fn new(
        bind_addr: SocketAddr,
        connect_addr: Option<SocketAddr>,
        buffer_size: usize,
    ) -> std::io::Result<Self> {
        let sock = UdpSocket::bind(bind_addr)?;
        if let Some(addr) = connect_addr {
            sock.connect(addr)?
        }
        Ok(Self {
            sock,
            buffer: vec![0u8; buffer_size],
        })
    }

    /// fetches a TFTP packet from the socket and returns it and the senders addres.
    pub fn get_next_message_from(&mut self) -> IoResult<(Packet<'_>, SocketAddr)> {
        let (n_bytes, client_addres) = self.sock.recv_from(&mut self.buffer)?;
        let message_buffer = &self.buffer[..n_bytes];
        Packet::from_bytes(message_buffer)
            .map_err(|err| {
                IoError::new(
                    std::io::ErrorKind::InvalidData,
                    format!("invalid packet received: {err:?}"),
                )
            })
            .map(|a| (a, client_addres))
    }

    /// sends a TFTP packet `message` to address `addr`
    pub fn send_message_to(&mut self, message: Packet, addr: SocketAddr) -> IoResult<()> {
        self.send_message_optionally_to(message, Some(addr))
    }

    /// sends a TFTP packet `message` to the address this socket is connected to.
    /// this method will fail if the socket is not connected to anything.
    pub fn send_message(&mut self, message: Packet) -> Result<(), IoError> {
        self.send_message_optionally_to(message, None)
    }

    /// sends a TFTP packet `message` to the given address or the default address this socket is connected to.
    /// this method will fail if no address is given and the socket is not connected to anything.
    pub fn send_message_optionally_to(
        &mut self,
        message: Packet,
        addr: Option<SocketAddr>,
    ) -> Result<(), IoError> {
        let bytes = message.to_bytes(&mut self.buffer).unwrap();
        let message = &self.buffer[..bytes];
        let bytes_send = if let Some(addr) = addr {
            UdpSocket::send_to(&self.sock, message, addr)
        } else {
            UdpSocket::send(&self.sock, message)
        }?;
        if bytes_send == message.len() {
            Ok(())
        } else {
            Err(IoError::new(
                std::io::ErrorKind::Other,
                format!("Failed to send UDP packet of size {bytes_send}"),
            ))
        }
    }
}
