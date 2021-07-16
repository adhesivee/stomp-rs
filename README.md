# stomp-rs

## Client
Creating new connection:
```rust
let client = Client::connect(
    ClientBuilder::new("127.0.0.1:61613".to_owned())
).await?;
```

Subscribing:
```rust
let (tx, mut rx) = channel();

client.subscribe("/topic/test".to_string(), None, tx)
    .await?;

if let Some(frame) = rx.recv().await {
    println!("Frame received: {}", frame.body);
}
```

Sending:
```rust
client.send("/topic/test".to_string(), "test-message".to_string())
    .await?;
```