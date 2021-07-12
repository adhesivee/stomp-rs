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
//! client.emit(
//!     Send::new("/topic/test".to_string())
//!         .body("test-payload".to_string())
//! );
//! ```
pub mod protocol;
pub mod client;
pub mod connection;

