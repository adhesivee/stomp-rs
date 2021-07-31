//! # Stomp lib
//!
//! ## Usage
//! Creating a client:
//! ```no_run
//! use stomp_rs::Client;
//! use tokio::net::TcpStream;
//!
//! let client = Client::new(
//!     TcpStream::connect("127.0.0.1:61613").await?,
//!     sender,
//! ).await?;
//! ```
//!
//! Emitting a new frame:
//!
//! ```no_run
//! use stomp_rs::protocol::frame::Send;
//!
//! client.send(
//!     "/topic/test".to_string(),
//!     "test-message".to_string(),
//!     None,
//! ).await?
//! ```
pub mod protocol;
pub mod client;
pub mod connection;

