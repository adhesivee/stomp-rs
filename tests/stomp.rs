use tokio::net::TcpListener;
use stomp_rs::client::{Client, ClientBuilder};
use std::error::Error;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::time::{sleep, Duration};
use std::io::{Write};
use stomp_rs::protocol::ServerCommand::Connected;
use stomp_rs::protocol::{Frame, ServerCommand};
use tokio::sync::mpsc::error::RecvError;

#[tokio::test]
async fn test_wrong_connect_response() -> Result<(), Box<dyn Error>> {
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let local_port = listener.local_addr().unwrap().port();

    tokio::spawn(async move {
        loop {
            let (mut socket, _) = listener.accept().await.unwrap();
            socket.writable().await.unwrap();
            socket.write_all("Invalid\n".as_bytes()).await.unwrap();
        }
    });


    sleep(Duration::from_secs(1)).await;
    let host = format!("127.0.0.1:{}", local_port);

    let client = Client::connect(
        ClientBuilder::new(host)
    ).await;

    assert!(client.is_err());
    Ok(())
}