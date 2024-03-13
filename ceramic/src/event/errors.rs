#[derive(Debug)]
pub enum EventError {
    MissingId,
    UnsupportedCodecError(u64),
    InvalidGenesisError,
    InvalidPreviousCid(String, String),
}

impl std::fmt::Display for EventError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EventError::MissingId => write!(f, "Missing Id in event"),
            EventError::UnsupportedCodecError(id) => write!(f, "Unsupported codec {}", id),
            EventError::InvalidGenesisError => write!(f, "invalid genesis event"),
            EventError::InvalidPreviousCid(prev, tip) => write!(f, "invalid prev cid: {} != {}", prev, tip),
        }
    }
}

impl std::error::Error for EventError {}