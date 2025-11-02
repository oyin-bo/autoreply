use thiserror::Error;

#[derive(Debug, Error)]
pub enum CarError {
    #[error("Unexpected end of data")]
    UnexpectedEof,
    #[error("Invalid CAR header: {0}")]
    InvalidHeader(String),
    #[error("Invalid CID version: {0}")]
    InvalidCidVersion(u8),
    #[error("Invalid CID codec: {0:#x}")]
    InvalidCidCodec(u8),

    #[error("UTF-8 decode error: {0}")]
    Utf8StringError(#[from] std::string::FromUtf8Error),
    #[error("Varint decode error: {0}")]
    VarintError(String),
    #[error("Invalid varint size")]
    InvalidVarintSize,
    #[error("Invalid digest size: expected {expected}, got {actual}")]
    InvalidDigestSize { expected: usize, actual: usize },
    #[error("UTF-8 decode error: {0}")]
    Utf8StrError(#[from] std::str::Utf8Error),
}
