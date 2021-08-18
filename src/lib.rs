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
//!
//! Subscribe:
//! ```no_run
//! use stomp_rs::client::Client;
//! use stomp_rs::protocol::frame::Subscribe;
//! use tokio::sync::mpsc::channel;
//! use std::error::Error;
//! use stomp_rs::protocol::{Frame, ServerCommand};
//! use std::future::Future;
//!
//! async fn subscribe_example(client: &Client)-> Result<(), Box<dyn Error>> {
//!   let (sender, mut receiver) = channel(16);
//!
//!   tokio::spawn(async move {
//!     match receiver.recv().await {
//!       Some(frame) => { /* process frame */}
//!       None => { }
//!     }
//!   });
//!
//!   client.subscribe(
//!       Subscribe::new_with_random_id("/topic/test"),
//!       sender
//!   ).await
//! }
pub mod client;
pub mod connection;
pub mod protocol;
