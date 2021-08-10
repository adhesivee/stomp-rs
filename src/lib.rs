//! # Stomp lib
//!
//! ## Usage
//! Creating a client:
//! ```no_run
//! use stomp_rs::client::{Client, ClientBuilder};
//! use tokio::net::TcpStream;
//! use tokio::sync::mpsc::channel;
//! use std::error::Error;
//!
//! async fn connect() -> Result<Client, Box<dyn Error>> {
//!   Client::connect(
//!       ClientBuilder::new("127.0.0.1:61613")
//!   ).await
//! }
//! ```
//!
//! Emitting a new frame:
//!
//! ```no_run
//! use stomp_rs::protocol::frame::Send;
//! use stomp_rs::client::Client;
//! use std::error::Error;;
//!
//! async fn send_example(client: &Client) -> Result<(), Box<dyn Error>> {
//!   client.send(
//!       Send::new("/topic/test")
//!         .body("test-message")
//!   ).await
//! }
//! ```
pub mod protocol;
pub mod client;
pub mod connection;

