pub mod frame;

use std::collections::HashMap;
use std::convert::TryFrom;
use std::error::Error;
use std::fmt::{Display, Formatter};

#[derive(Debug, PartialEq, Copy, Clone)]
pub enum ClientCommand {
    Connect,
    Send,
    Subscribe,
    Unsubscribe,
    Ack,
    Nack,
    Begin,
    Commit,
    Abort,
    Disconnect,
}

#[derive(Debug, PartialEq, Copy, Clone)]
pub enum ServerCommand {
    Connected,
    Message,
    Receipt,
    Error,
}

impl From<ServerCommand> for &str {
    fn from(value: ServerCommand) -> Self {
        match value {
            ServerCommand::Connected => "CONNECTED",
            ServerCommand::Message => "MESSAGE",
            ServerCommand::Receipt => "RECEIPT",
            ServerCommand::Error => "ERROR",
        }
    }
}

impl From<ClientCommand> for &str {
    fn from(value: ClientCommand) -> Self {
        match value {
            ClientCommand::Connect => "CONNECT",
            ClientCommand::Send => "SEND",
            ClientCommand::Subscribe => "SUBSCRIBE",
            ClientCommand::Unsubscribe => "UNSUBSCRIBE",
            ClientCommand::Ack => "ACK",
            ClientCommand::Nack => "NACK",
            ClientCommand::Begin => "BEGIN",
            ClientCommand::Commit => "COMMIT",
            ClientCommand::Abort => "ABORT",
            ClientCommand::Disconnect => "DISCONNECT",
        }
    }
}

impl TryFrom<&str> for ClientCommand {
    type Error = &'static str;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "CONNECT" => Ok(ClientCommand::Connect),
            "SEND" => Ok(ClientCommand::Send),
            "SUBSCRIBE" => Ok(ClientCommand::Subscribe),
            "UNSUBSCRIBE" => Ok(ClientCommand::Unsubscribe),
            "ACK" => Ok(ClientCommand::Ack),
            "NACK" => Ok(ClientCommand::Nack),
            "BEGIN" => Ok(ClientCommand::Begin),
            "COMMIT" => Ok(ClientCommand::Commit),
            "ABORT" => Ok(ClientCommand::Abort),
            "DISCONNECT" => Ok(ClientCommand::Disconnect),
            _ => Err("Unknown client command"),
        }
    }
}

impl TryFrom<&str> for ServerCommand {
    type Error = &'static str;

    fn try_from(value: &str) -> Result<Self, <ServerCommand as TryFrom<&'static str>>::Error> {
        match value {
            "CONNECTED" => Ok(ServerCommand::Connected),
            "MESSAGE" => Ok(ServerCommand::Message),
            "RECEIPT" => Ok(ServerCommand::Receipt),
            "ERROR" => Ok(ServerCommand::Error),
            _ => Err("Unknown client command"),
        }
    }
}

impl Command for ServerCommand {}

impl Command for ClientCommand {}

#[derive(Debug, Clone)]
pub struct Frame<T>
where
    T: Into<&'static str>,
{
    pub command: T,
    pub headers: HashMap<String, String>,
    pub body: String,
}

impl<T> Frame<T>
where
    T: Into<&'static str> + Copy,
{
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut buffer = vec![];

        buffer.extend_from_slice(self.command.into().as_bytes());
        buffer.push(BNF_LF);

        self.headers.iter().for_each(|entry| {
            buffer.extend_from_slice(entry.0.as_bytes());
            buffer.extend_from_slice(":".as_bytes());
            buffer.extend_from_slice(entry.1.as_bytes());
            buffer.push(BNF_LF)
        });

        buffer.push(BNF_LF);
        buffer.extend_from_slice(self.body.as_bytes());
        buffer.push(BNF_NULL);

        buffer
    }
}

#[derive(PartialEq)]
enum ReadingState {
    Command,
    Header,
    Body,
    Completed,
}

const BNF_NULL: u8 = 0;
pub(crate) const BNF_LF: u8 = 10;
const BNF_CR: u8 = 13;

pub trait Command: Into<&'static str> + for<'a> TryFrom<&'a str> {}

pub struct FrameParser<T: Command> {
    buffer: Vec<u8>,
    state: ReadingState,

    current_command: Option<T>,
    current_headers: Option<HashMap<String, String>>,
}

#[derive(Debug, Clone)]
pub enum StompMessage<T: Command + Clone> {
    Frame(Frame<T>),
    Ping,
}

#[derive(Debug)]
pub enum ParseError {
    CommandNotFound(String),
}

impl Display for ParseError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "Parsing error")
    }
}

impl Error for ParseError {}

impl<T: Command + Clone> FrameParser<T> {
    pub fn new() -> FrameParser<T> {
        FrameParser {
            buffer: vec![],
            state: ReadingState::Command,
            current_command: None,
            current_headers: None,
        }
    }

    pub fn parse(&mut self, body: &[u8]) -> Result<Vec<StompMessage<T>>, ParseError> {
        let mut frames = vec![];

        let mut body_slice = &body[..];

        loop {
            let collect_until = match self.state {
                ReadingState::Command => BNF_LF,
                ReadingState::Header => BNF_LF,
                ReadingState::Body => BNF_NULL,
                ReadingState::Completed => BNF_LF,
            };

            let position = &body_slice.iter().position(|b| *b == collect_until);

            match position {
                Some(position) => {
                    let previous_position = position.saturating_sub(1_usize);

                    let buffer_until = if collect_until == BNF_LF
                        && body_slice
                            .get(previous_position)
                            .iter()
                            .all(|b| **b == BNF_CR)
                    {
                        previous_position
                    } else {
                        *position
                    };

                    self.buffer.extend(&body_slice[..buffer_until]);
                    body_slice = &body_slice[(u32::try_from(*position).unwrap() + 1) as usize..];
                }
                None => {
                    self.buffer.extend(&body_slice[..]);
                    break;
                }
            }

            if let ReadingState::Completed = self.state {
                if !self.buffer.is_empty() && self.buffer.iter().any(|b| *b != BNF_CR) {
                    self.state = ReadingState::Command;
                }
            }

            match self.state {
                ReadingState::Command => {
                    let buffer = std::mem::take(&mut self.buffer);
                    let command_string = String::from_utf8(buffer).unwrap();

                    let command = T::try_from(&command_string);

                    self.current_command = match command {
                        Ok(value) => Some(value),
                        Err(_) => {
                            return Err(ParseError::CommandNotFound(command_string.to_string()));
                        }
                    };

                    self.state = ReadingState::Header;
                    self.current_headers = Some(HashMap::new());
                }
                ReadingState::Header => {
                    if self.buffer.is_empty() {
                        self.state = ReadingState::Body;
                    } else {
                        let buffer = std::mem::take(&mut self.buffer);
                        let header_line = String::from_utf8(buffer).unwrap();
                        let mut header = header_line.split(':');
                        self.current_headers.as_mut().unwrap().insert(
                            header.next().unwrap().trim().to_string(),
                            header.next().unwrap().trim().to_string(),
                        );
                    }
                }
                ReadingState::Body => {
                    let buffer = std::mem::take(&mut self.buffer);
                    let body = String::from_utf8(buffer).unwrap();

                    self.state = ReadingState::Completed;

                    let frame_command = std::mem::take(&mut self.current_command);
                    let frame_headers = std::mem::take(&mut self.current_headers);

                    frames.push(StompMessage::Frame(Frame {
                        command: frame_command.unwrap(),
                        headers: frame_headers.unwrap(),
                        body,
                    }));
                }
                ReadingState::Completed => {
                    frames.push(StompMessage::Ping);
                    self.buffer.clear();
                }
            }
        }

        Ok(frames)
    }
}

#[cfg(test)]
mod test {
    use crate::protocol::{ClientCommand, FrameParser, StompMessage};

    #[tokio::test]
    async fn parse_test() {
        let body = "SEND\n\
        test: value\n\
        test_val: heeerre\n\
        \n\
        body\n\
        first body\0\n\n\
        \n\
        \n\
        SEND\n\
        test2: value\n\
        \n\
        body : test\n\
        second body\0
        "
        .as_bytes();

        let mut frames = vec![];
        let mut parser: FrameParser<ClientCommand> = FrameParser::new();

        for body_chunk in body.chunks(4) {
            frames.append(&mut parser.parse(body_chunk).unwrap());
        }

        let frame = frames.first();

        assert!(frame.is_some());
        let frame = frame.unwrap();

        if let StompMessage::Frame(frame) = frame {
            assert_eq!(frame.command, ClientCommand::Send);
            let headers = &frame.headers;

            assert!(headers.contains_key("test"));
            assert_eq!(headers.get("test").unwrap(), "value");
            assert!(headers.contains_key("test_val"));
            assert_eq!(headers.get("test_val").unwrap(), "heeerre");

            assert_eq!(
                frame.body,
                "body\n\
first body"
            );
            println!("{:?}", frame);
        }
    }

    #[tokio::test]
    async fn parse_test_cr() {
        let body = "SEND\r\n\
        test: value\r\n\
        test_val: heeerre\r\n\
        \r\n\
        body\r\n\
        first body\0\n\n\
        \r\n\
        \r\n\
        SEND\n\
        test2: value\n\
        \n\
        body : test\n\
        second body\0
        "
        .as_bytes();

        let mut frames = vec![];
        let mut parser: FrameParser<ClientCommand> = FrameParser::new();

        for body_chunk in body.chunks(4) {
            frames.append(&mut parser.parse(body_chunk).unwrap());
        }

        let frame = frames.first();

        assert!(frame.is_some());
        let frame = frame.unwrap();

        if let StompMessage::Frame(frame) = frame {
            assert_eq!(frame.command, ClientCommand::Send);
            let headers = &frame.headers;

            assert!(headers.contains_key("test"));
            assert_eq!(headers.get("test").unwrap(), "value");
            assert!(headers.contains_key("test_val"));
            assert_eq!(headers.get("test_val").unwrap(), "heeerre");

            assert_eq!(
                frame.body,
                "body\r\n\
first body"
            );
            println!("{:?}", frame);
        }
    }
}
