#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(feature = "std")]
pub mod datastream;
pub mod error;
pub mod packet;
#[cfg(feature = "std")]
pub mod server;
#[cfg(feature = "std")]
pub mod socket;

pub use packet::Packet;
