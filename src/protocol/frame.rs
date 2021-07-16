use crate::protocol::{Frame, ClientCommand};
use std::collections::HashMap;

pub struct Nack {
    headers: HashMap<String, String>
}

impl Nack {
    pub fn new(id: String) -> Self {
        let mut headers = HashMap::new();
        headers.insert("id".to_string(), id);

        Self {
            headers
        }
    }

    pub fn transaction(self, tx_id: String) -> Self {
        self.header("transaction".to_string(), tx_id)
    }

    pub fn header(mut self, key: String, value: String) -> Self {
        self.headers.insert(key, value);

        self
    }
}

impl Into<Frame<ClientCommand>> for Nack {
    fn into(self) -> Frame<ClientCommand> {
        Frame {
            command: ClientCommand::Ack,
            headers: self.headers,
            body: "".to_string()
        }
    }
}

pub struct Ack {
    headers: HashMap<String, String>
}

impl Ack {
    pub fn new(id: String) -> Self {
        let mut headers = HashMap::new();
        headers.insert("id".to_string(), id);

        Ack {
            headers
        }
    }

    pub fn transaction(self, tx_id: String) -> Self {
        self.header("transaction".to_string(), tx_id)
    }

    pub fn header(mut self, key: String, value: String) -> Self {
        self.headers.insert(key, value);

        self
    }
}

impl Into<Frame<ClientCommand>> for Ack {
    fn into(self) -> Frame<ClientCommand> {
        Frame {
            command: ClientCommand::Ack,
            headers: self.headers,
            body: "".to_string()
        }
    }
}

pub struct Connect {
    headers: HashMap<String, String>
}

impl Connect {
    pub fn new(accept_version: String, host: String) -> Self {
        let mut headers = HashMap::new();
        headers.insert("accept-version".to_string(), accept_version);
        headers.insert("host".to_string(), host);

        Connect { headers }
    }

    pub fn heartbeat(self, client_interval: u32, server_interval: u32) -> Self {
        self.header("heart-beat".to_string(), format!("{},{}", client_interval, server_interval))
    }

    pub fn header(mut self, key: String, value: String) -> Self {
        self.headers.insert(key, value);

        self
    }
}

impl Into<Frame<ClientCommand>> for Connect {
    fn into(self) -> Frame<ClientCommand> {
        Frame {
            command: ClientCommand::Connect,
            headers: self.headers,
            body: "".to_string()
        }
    }
}

pub struct Send {
    headers: HashMap<String, String>,
    payload: String
}

impl Send {
    pub fn new(destination: String) -> Self {
        let mut headers = HashMap::new();
        headers.insert("destination".to_string(), destination);

        Send { headers, payload: "".to_string() }
    }

    pub fn body(mut self, payload: String) -> Self {
        self.payload = payload;

        self
    }

    pub fn receipt(self, receipt_id: String) -> Self {
        self.header("receipt".to_string(), receipt_id)
    }

    pub fn header(mut self, key: String, value: String) -> Self {
        self.headers.insert(key, value);

        self
    }
}

impl Into<Frame<ClientCommand>> for Send {
    fn into(self) -> Frame<ClientCommand> {
        Frame {
            command: ClientCommand::Send,
            headers: self.headers,
            body: self.payload
        }
    }
}

pub struct Subscribe {
    headers: HashMap<String, String>
}

impl Subscribe {
    pub fn new(id: String, destination: String) -> Self {
        let mut headers = HashMap::new();
        headers.insert("id".to_string(), id);
        headers.insert("destination".to_string(), destination);

        Subscribe {
            headers
        }
    }

    pub fn receipt(self, receipt_id: String) -> Self {
        self.header("receipt".to_string(), receipt_id)
    }

    pub fn header(mut self, key: String, value: String) -> Self {
        self.headers.insert(key, value);

        self
    }
}

impl Into<Frame<ClientCommand>> for Subscribe {
    fn into(self) -> Frame<ClientCommand> {
        Frame {
            command: ClientCommand::Subscribe,
            headers: self.headers,
            body: "".to_string()
        }
    }
}


pub struct Unsubscribe {
    headers: HashMap<String, String>
}

impl Unsubscribe {
    pub fn new(id: String) -> Self {
        let mut headers = HashMap::new();
        headers.insert("id".to_string(), id);

        Self {
            headers
        }
    }

    pub fn receipt(self, receipt_id: String) -> Self {
        self.header("receipt".to_string(), receipt_id)
    }

    pub fn header(mut self, key: String, value: String) -> Self {
        self.headers.insert(key, value);

        self
    }
}

impl Into<Frame<ClientCommand>> for Unsubscribe {
    fn into(self) -> Frame<ClientCommand> {
        Frame {
            command: ClientCommand::Unsubscribe,
            headers: self.headers,
            body: "".to_string()
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::protocol::frame::{Ack, Nack, Connect, Send, Subscribe};
    use crate::protocol::{Frame, ClientCommand};

    #[test]
    fn test_ack() {
        let ack_id = "12345";
        let test_header = "random";
        let test_value = "54321";

        let frame: Frame<ClientCommand> = Ack::new(ack_id.to_owned())
            .header(test_header.to_owned(), test_value.to_owned())
            .into();

        assert_eq!(frame.command, ClientCommand::Ack);
        assert_eq!(frame.headers["id"], ack_id);
        assert_eq!(frame.headers[test_header], test_value);
    }

    #[test]
    fn test_nack() {
        let nack_id = "12345";
        let test_header = "random";
        let test_value = "54321";

        let frame: Frame<ClientCommand> = Nack::new(nack_id.to_owned())
            .header(test_header.to_owned(), test_value.to_owned())
            .into();

        assert_eq!(frame.command, ClientCommand::Ack);
        assert_eq!(frame.headers["id"], nack_id);
        assert_eq!(frame.headers[test_header], test_value);
    }

    #[test]
    fn test_connect() {
        let accept_version = "1.2";
        let client_heartbeat = 10;
        let server_heartbeat = 25;
        let host = "test-host";
        let test_header = "random";
        let test_value = "54321";

        let frame: Frame<ClientCommand> = Connect::new(accept_version.to_owned(), host.to_owned())
            .heartbeat(client_heartbeat, server_heartbeat)
            .header(test_header.to_owned(), test_value.to_owned())
            .into();

        assert_eq!(frame.command, ClientCommand::Connect);
        assert_eq!(frame.headers["accept-version"], accept_version);
        assert_eq!(frame.headers["host"], host);
        assert_eq!(frame.headers["heart-beat"], format!("{},{}", client_heartbeat, server_heartbeat));
        assert_eq!(frame.headers[test_header], test_value);
    }

    #[test]
    fn test_send() {
        let destination = "/dest/123";
        let body = "test-payload";
        let test_header = "random";
        let test_value = "54321";

        let frame: Frame<ClientCommand> = Send::new(destination.to_owned())
            .body(body.to_owned())
            .header(test_header.to_owned(), test_value.to_owned())
            .into();

        assert_eq!(frame.command, ClientCommand::Send);
        assert_eq!(frame.headers["destination"], destination);
        assert_eq!(frame.body, body);
        assert_eq!(frame.headers[test_header], test_value);
    }

    #[test]
    fn test_subscribe() {
        let subscribe_id = "12345";
        let destination = "/dest/123";
        let receipt_id = "receipt-123";
        let test_header = "random";
        let test_value = "54321";

        let frame: Frame<ClientCommand> = Subscribe::new(subscribe_id.to_owned(), destination.to_owned())
            .receipt(receipt_id.to_owned())
            .header(test_header.to_owned(), test_value.to_owned())
            .into();

        assert_eq!(frame.command, ClientCommand::Subscribe);
        assert_eq!(frame.headers["id"], subscribe_id);
        assert_eq!(frame.headers["destination"], destination);
        assert_eq!(frame.headers["receipt"], receipt_id);
        assert_eq!(frame.headers[test_header], test_value);
    }
}