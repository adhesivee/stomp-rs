use log::{debug, LevelFilter};
use simple_logger::SimpleLogger;
use std::collections::HashMap;
use std::error::Error;
use std::io::Write;
use stomp_rs::client::{Client, ClientBuilder};
use stomp_rs::protocol::frame::{Send, Subscribe};
use stomp_rs::protocol::ServerCommand::Connected;
use stomp_rs::protocol::{ClientCommand, Frame, FrameParser, ServerCommand, StompMessage};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::sync::mpsc::channel;
use tokio::sync::mpsc::error::RecvError;
use tokio::time::{sleep, Duration, Instant};

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

    let client = Client::connect(ClientBuilder::new(host)).await;

    assert!(client.is_err());
    Ok(())
}

#[tokio::test]
async fn test_heartbeat_closing_client() -> Result<(), Box<dyn Error>> {
    SimpleLogger::new().with_level(LevelFilter::Debug).init();

    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let local_port = listener.local_addr().unwrap().port();

    tokio::spawn(async move {
        loop {
            let (mut socket, _) = listener.accept().await.unwrap();
            debug!("Wait for writeable");
            socket.writable().await.unwrap();

            sleep(Duration::from_secs(5)).await;
        }
    });

    sleep(Duration::from_millis(10)).await;
    let host = format!("127.0.0.1:{}", local_port);

    let started = Instant::now();
    let client = Client::connect(ClientBuilder::new(host).heartbeat(500, 500)).await;

    assert!(client.is_err());
    assert!(started.elapsed().as_millis() < 1000);
    Ok(())
}

#[tokio::test]
async fn test_proper_connection() -> Result<(), Box<dyn Error>> {
    SimpleLogger::new().with_level(LevelFilter::Debug).init();

    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let local_port = listener.local_addr().unwrap().port();

    tokio::spawn(async move {
        let mut parser: FrameParser<ClientCommand> = FrameParser::new();

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
                                socket
                                    .write_all(
                                        &Frame {
                                            command: ServerCommand::Connected,
                                            headers: Default::default(),
                                            body: "".to_string(),
                                        }
                                        .to_bytes(),
                                    )
                                    .await
                                    .unwrap();
                            }
                        }
                    }
                }
                Err(_) => {
                    panic!("Could not parse frame")
                }
            }
        }
    });

    sleep(Duration::from_millis(10)).await;
    let host = format!("127.0.0.1:{}", local_port);

    Client::connect(ClientBuilder::new(host)).await.map(|_| ())
}

#[tokio::test]
async fn test_subscribe_message() -> Result<(), Box<dyn Error>> {
    SimpleLogger::new().with_level(LevelFilter::Debug).init();

    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let local_port = listener.local_addr().unwrap().port();

    tokio::spawn(async move {
        let mut parser: FrameParser<ClientCommand> = FrameParser::new();
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
                                    socket
                                        .write_all(
                                            &Frame {
                                                command: ServerCommand::Connected,
                                                headers: Default::default(),
                                                body: "".to_string(),
                                            }
                                            .to_bytes(),
                                        )
                                        .await
                                        .unwrap();
                                }
                                ClientCommand::Subscribe => {
                                    let mut headers = HashMap::new();
                                    headers.insert(
                                        "receipt-id".to_string(),
                                        frame.headers["receipt"].to_string(),
                                    );

                                    debug!("Write receipt with id: {}", frame.headers["receipt"]);
                                    socket
                                        .write_all(
                                            &Frame {
                                                command: ServerCommand::Receipt,
                                                headers,
                                                body: "".to_string(),
                                            }
                                            .to_bytes(),
                                        )
                                        .await;
                                }
                                ClientCommand::Send => {
                                    let mut headers = HashMap::new();
                                    headers.insert(
                                        "receipt-id".to_string(),
                                        frame.headers["receipt"].to_string(),
                                    );

                                    debug!("Write receipt with id: {}", frame.headers["receipt"]);
                                    socket
                                        .write_all(
                                            &Frame {
                                                command: ServerCommand::Receipt,
                                                headers,
                                                body: "".to_string(),
                                            }
                                            .to_bytes(),
                                        )
                                        .await;

                                    debug!("Write message");
                                    let mut headers = frame.headers.clone();
                                    headers.insert("subscription".to_string(), "1".to_string());
                                    socket
                                        .write_all(
                                            &Frame {
                                                command: ServerCommand::Message,
                                                headers,
                                                body: "".to_string(),
                                            }
                                            .to_bytes(),
                                        )
                                        .await;
                                }
                                _ => {}
                            }
                        }
                    }
                }
                Err(_) => {
                    panic!("Could not parse frame")
                }
            }
        }
    });

    sleep(Duration::from_millis(10)).await;
    let host = format!("127.0.0.1:{}", local_port);

    let client = Client::connect(ClientBuilder::new(host)).await?;

    debug!("Sent message");
    let time = Instant::now();

    let (sender, mut receiver) = channel(1);
    client
        .subscribe(Subscribe::new("1", "/test/topic"), sender)
        .await
        .unwrap();

    let handle = tokio::spawn(async move {
        let frame = receiver.recv().await.unwrap();

        frame
    });
    let send = client.send(Send::new("/test/topic")).await?;

    debug!("Message send");
    let frame = handle.await?;

    assert_eq!(frame.headers["destination"], "/test/topic");

    Ok(())
}

#[tokio::test]
async fn test_await_receipt() -> Result<(), Box<dyn Error>> {
    SimpleLogger::new().with_level(LevelFilter::Debug).init();

    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let local_port = listener.local_addr().unwrap().port();

    tokio::spawn(async move {
        let mut parser: FrameParser<ClientCommand> = FrameParser::new();
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
                                    socket
                                        .write_all(
                                            &Frame {
                                                command: ServerCommand::Connected,
                                                headers: Default::default(),
                                                body: "".to_string(),
                                            }
                                            .to_bytes(),
                                        )
                                        .await
                                        .unwrap();
                                }
                                ClientCommand::Send => {
                                    debug!("Wait before answer");
                                    sleep(Duration::from_secs(1)).await;

                                    let mut headers = HashMap::new();
                                    headers.insert(
                                        "receipt-id".to_string(),
                                        frame.headers["receipt"].to_string(),
                                    );

                                    debug!("Write receipt with id: {}", frame.headers["receipt"]);
                                    socket
                                        .write_all(
                                            &Frame {
                                                command: ServerCommand::Receipt,
                                                headers,
                                                body: "".to_string(),
                                            }
                                            .to_bytes(),
                                        )
                                        .await;
                                }
                                _ => {}
                            }
                        }
                    }
                }
                Err(_) => {
                    panic!("Could not parse frame")
                }
            }
        }
    });

    sleep(Duration::from_millis(10)).await;
    let host = format!("127.0.0.1:{}", local_port);

    let client = Client::connect(ClientBuilder::new(host)).await?;

    debug!("Sent message");
    let time = Instant::now();
    let send = client.send(Send::new("/test/topic")).await?.notify.notified().await;

    debug!("Message send");
    assert!(time.elapsed().as_millis() >= 1000);

    Ok(())
}
