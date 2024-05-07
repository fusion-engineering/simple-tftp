#![feature(doc_cfg)]
#![cfg_attr(not(feature = "std"), no_std)]
#![warn(missing_docs)]

//! A crate that contains everything you need to build a small TFTP server or client.
//! It aims to be easy to use / read over over performant.
//! # What is TFTP?
//! TFTP is an older protocol for transfering files over the network defined in [RFC-1350: The TFTP Protocol (Revision 2)](https://www.rfc-editor.org/rfc/inline-errata/rfc1350.html). These days it is mainly used to boot or flash embedded devices over ethernet.
//!
//!# Supported RFCs
//! ✅ [1350 - The TFTP Protocol (Revision 2)](https://www.rfc-editor.org/rfc/inline-errata/rfc1350.html)
//!
//! ✅ [2347 - TFTP Option Extension](https://www.rfc-editor.org/rfc/inline-errata/rfc2347.html)
//!
//! ✅ [2348 - TFTP Blocksize Option](https://www.rfc-editor.org/rfc/rfc2348.html)
//!
//! ⚠️ [2349 - TFTP Timeout Interval and Transfer Size Options](https://www.rfc-editor.org/rfc/rfc2349.html)
//!
//! ╰Timeout option is recognized by the packet parser, but not supported by the server.
//!
//! ❌ [2090 - TFTP Multicast Option](https://www.rfc-editor.org/rfc/rfc2090.html)
//!
//!# `#[no_std]` support
//! This crate is `#[no_std]` by default, exposing only packet and error handling code.
//! With the `std` feature turned on a small socket interface and server are enabled too.
#[cfg(feature = "std")]
mod datastream;
/// error types for this crate
pub mod error;
/// all type definitions needed to parse TFTP packets
pub mod packet;
/// a small server implementation
#[cfg(feature = "std")]
#[doc(cfg(feature = "std"))]
pub mod server;
#[cfg(feature = "std")]
#[doc(cfg(feature = "std"))]
/// A wrapper around a UDP socket that can be used to build a client or server,
pub mod socket;

pub use error::Result;
pub use packet::Packet;
