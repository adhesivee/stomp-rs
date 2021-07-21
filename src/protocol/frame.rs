use crate::protocol::{Frame, ClientCommand};
use std::collections::HashMap;

macro_rules! default_frame {
    ($struct:ident($($initname:ident),*) => $command:expr => $($method:ident ($($names:ident),+)),+) => {
        pub struct $struct {
            headers: HashMap<String, String>
        }

        impl $struct {
            pub fn new($($initname: String),*) -> Self {
                let mut headers = HashMap::new();
                $(headers.insert(stringify!($initname).to_string().replace('_', "-"), $initname);)*

                Self {
                    headers
                }
            }

            $(pub fn $method(self, $($names: String),+) -> Self {{
                let mut current = self;
                $(current = current.header(stringify!($names).to_string().replace('_', "-"), $names);),+

                current
            }}),+

            pub fn header(mut self, key: String, value: String) -> Self {
                self.headers.insert(key, value);

                self
            }
        }

        impl Into<Frame<ClientCommand>> for $struct {
            fn into(self) -> Frame<ClientCommand> {
                Frame {
                    command: $command,
                    headers: self.headers,
                    body: "".to_string()
                }
            }
        }
    }
}


default_frame!(
    Nack(id) => ClientCommand::Nack =>
        transaction (transaction)
);

default_frame!(
    Ack(id) => ClientCommand::Ack =>
        transaction (transaction)
);

default_frame!(
    Connect(accept_version, host) => ClientCommand::Connect =>
        test (val)
);

impl Connect {
    pub fn heartbeat(self, client_interval: u32, server_interval: u32) -> Self {
        self.header("heart-beat".to_string(), format!("{},{}", client_interval, server_interval))
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

default_frame!(
    Subscribe(id, destination) => ClientCommand::Subscribe =>
        receipt (receipt)
);

default_frame!(
    Unsubscribe(id) => ClientCommand::Unsubscribe =>
        receipt (receipt)
);

default_frame!(
    Begin(transaction) => ClientCommand::Begin =>
        receipt (receipt)
);

default_frame!(
    Commit(transaction) => ClientCommand::Commit =>
        receipt (receipt)
);

default_frame!(
    Abort(transaction) => ClientCommand::Abort =>
        receipt (receipt)
);

default_frame!(
    Disconnect(transaction) => ClientCommand::Disconnect =>
        receipt (receipt)
);

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

        assert_eq!(frame.command, ClientCommand::Nack);
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