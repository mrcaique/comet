#[derive(Debug)]
pub enum MessageHandlingErrorKind {
    NotImplemented,
    IO(tokio::io::Error),
    Proto(protobuf::Error),
}

#[derive(Debug)]
pub struct MessageHandlingError {
    pub kind: MessageHandlingErrorKind,
}

impl std::error::Error for MessageHandlingError {}

impl MessageHandlingError {
    pub fn new(kind: MessageHandlingErrorKind) -> MessageHandlingError {
        MessageHandlingError { kind }
    }
}

impl std::fmt::Display for MessageHandlingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "")
    }
}
