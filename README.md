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
client.subscribe("/topic/test".to_string())
    .await?;
```