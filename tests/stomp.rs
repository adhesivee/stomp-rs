use tokio::net::TcpListener;
use stomp_rs::client::{Client, ClientBuilder};
use std::error::Error;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::time::{sleep, Duration, Instant};
use std::io::{Write};
use stomp_rs::protocol::frame::Send;
use stomp_rs::protocol::ServerCommand::Connected;
use stomp_rs::protocol::{StompMessage, Frame, ServerCommand, FrameParser, ClientCommand};
use tokio::sync::mpsc::error::RecvError;
use log::{debug, LevelFilter};
use simple_logger::SimpleLogger;
use std::collections::HashMap;

#[tokio::test]
async fn test_wrong_connect_response() -> Result<(), Box<dyn Error>> {
    SimpleLogger::new().with_level(LevelFilter::Debug).init();

    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let local_port = listener.local_addr().unwrap().port();

    tokio::spawn(async move {
        loop {
            let (mut socket, _) = listener.accept().await.unwrap();
            debug!("Wait for writeable");
            socket.writable().await.unwrap();
            debug!("Write invalid frame");
            socket.write_all("Invalid\n".as_bytes()).await.unwrap();
        }
    });

    sleep(Duration::from_millis(10)).await;
    let host = format!("127.0.0.1:{}", local_port);

    let client = Client::connect(
        ClientBuilder::new(host)
    ).await;

    assert!(client.is_err());
    Ok(())
}

#[tokio::test]
async fn test_proper_connection() -> Result<(), Box<dyn Error>> {
    SimpleLogger::new().with_level(LevelFilter::Debug).init();

    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let local_port = listener.local_addr().unwrap().port();

    tokio::spawn(async move {
        let mut parser : FrameParser<ClientCommand> = FrameParser::new();

        loop {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut buffer = vec![0; 8096];

            let size = socket.read(&mut buffer).await.unwrap();
            match parser.parse(&buffer[..size]) {
                Ok(messages) => {
                    for message in messages {
                        if let StompMessage::Frame(frame) = message {
                            if let ClientCommand::Connect = frame.command {
                                eprintln!("Write connected");
                                socket.write_all(
                                    &Frame {
                                        command: ServerCommand::Connected,
                                        headers: Default::default(),
                                        body: "".to_string()
                                    }.to_bytes()
                                ).await.unwrap();
                            }
                        }
                    }
                }
                Err(_) => { panic!("Could not parse frame") }
            }
        }
    });


    sleep(Duration::from_millis(10)).await;
    let host = format!("127.0.0.1:{}", local_port);

    Client::connect(ClientBuilder::new(host))
        .await
        .map(|_| ())
}

#[tokio::test]
async fn test_await_receipt() -> Result<(), Box<dyn Error>> {
    SimpleLogger::new().with_level(LevelFilter::Debug).init();

    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let local_port = listener.local_addr().unwrap().port();

    tokio::spawn(async move {
        let mut parser : FrameParser<ClientCommand> = FrameParser::new();
        let (mut socket, _) = listener.accept().await.unwrap();

        loop {
            let mut buffer = vec![0; 8096];

            let size = socket.read(&mut buffer).await.unwrap();
            match parser.parse(&buffer[..size]) {
                Ok(messages) => {
                    for message in messages {
                        if let StompMessage::Frame(frame) = message {
                            match frame.command {
                                ClientCommand::Connect => {
                                    debug!("Write connected");
                                    socket.write_all(
                                        &Frame {
                                            command: ServerCommand::Connected,
                                            headers: Default::default(),
                                            body: "".to_string()
                                        }.to_bytes()
                                    ).await.unwrap();
                                }
                                ClientCommand::Send => {
                                    debug!("Wait before answer");
                                    sleep(Duration::from_secs(1)).await;

                                    let mut headers = HashMap::new();
                                    headers.insert("receipt-id".to_string(), frame.headers["receipt"].to_string());

                                    debug!("Write receipt with id: {}", frame.headers["receipt"]);
                                    socket.write_all(
                                        &Frame {
                                            command: ServerCommand::Receipt,
                                            headers,
                                            body: "".to_string()
                                        }.to_bytes()
                                    ).await;
                                }
                                _ => {}
                            }
                        }
                    }
                }
                Err(_) => { panic!("Could not parse frame") }
            }
        }
    });


    sleep(Duration::from_millis(10)).await;
    let host = format!("127.0.0.1:{}", local_port);

    let client = Client::connect(ClientBuilder::new(host))
        .await?;

    debug!("Sent message");
    let time = Instant::now();
    let send = client.send(Send::new("/test/topic"))
        .await?;

    debug!("Message send");
    assert!(time.elapsed().as_millis() >= 1000);

    Ok(())
}