# stomp-rs

## Client
Creating new connection:
```rust
let client = Client::connect(
    ClientBuilder::new("127.0.0.1:61613")
).await?;
```

Subscribing:
```rust
let (sender, mut receiver) = channel(16);

tokio::spawn(async move {
  match receiver.recv().await {
    Some(frame) => { /* process frame */}
    None => { }
  }
});
client.subscribe(
    Subscribe::new_with_random_id("/topic/test"),
    sender
).await
```

Sending:
```rust
client.send(
    Send::new("/topic/test")
      .body("test-message")
).await
```

Transaction:
```rust
let transaction = client.begin().await?;

transaction.send(
    Send::new("/topic/test")
      .body("test-message")
).await
```