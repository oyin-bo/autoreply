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

    #[error("CBOR decode error: {0}")]
    CborError(#[from] serde_cbor::Error),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_car_error_display() {
        let error = CarError::UnexpectedEof;
        assert_eq!(error.to_string(), "Unexpected end of data");

        let error = CarError::InvalidHeader("test error".to_string());
        assert_eq!(error.to_string(), "Invalid CAR header: test error");

        let error = CarError::InvalidCidVersion(2);
        assert_eq!(error.to_string(), "Invalid CID version: 2");

        let error = CarError::InvalidCidCodec(0x99);
        assert_eq!(error.to_string(), "Invalid CID codec: 0x99");

        let error = CarError::VarintError("test varint error".to_string());
        assert_eq!(error.to_string(), "Varint decode error: test varint error");

        let error = CarError::InvalidVarintSize;
        assert_eq!(error.to_string(), "Invalid varint size");

        let error = CarError::InvalidDigestSize {
            expected: 32,
            actual: 16,
        };
        assert_eq!(
            error.to_string(),
            "Invalid digest size: expected 32, got 16"
        );
    }

    #[test]
    fn test_car_error_from_conversions() {
        // Test CBOR error conversion
        let invalid_cbor = b"\xFF\xFF\xFF";
        let cbor_error = serde_cbor::from_slice::<serde_cbor::Value>(invalid_cbor).unwrap_err();
        let car_error: CarError = cbor_error.into();
        assert!(matches!(car_error, CarError::CborError(_)));

        // Test UTF-8 string error conversion
        let invalid_utf8 = vec![0xFF, 0xFE, 0xFD];
        let utf8_error = String::from_utf8(invalid_utf8).unwrap_err();
        let car_error: CarError = utf8_error.into();
        assert!(matches!(car_error, CarError::Utf8StringError(_)));

        // Test UTF-8 str error conversion
        // Create invalid UTF-8 via opaque function to avoid compiler warning
        fn create_invalid_utf8() -> Vec<u8> {
            vec![0xFF, 0xFE, 0xFD] // Invalid UTF-8 sequence
        }
        let invalid_utf8_bytes = create_invalid_utf8();
        let utf8_str_error = std::str::from_utf8(&invalid_utf8_bytes).unwrap_err();
        let car_error: CarError = utf8_str_error.into();
        assert!(matches!(car_error, CarError::Utf8StrError(_)));
    }

    #[test]
    fn test_car_error_debug() {
        let error = CarError::UnexpectedEof;
        let debug_str = format!("{:?}", error);
        assert_eq!(debug_str, "UnexpectedEof");

        let error = CarError::InvalidDigestSize {
            expected: 32,
            actual: 16,
        };
        let debug_str = format!("{:?}", error);
        assert_eq!(debug_str, "InvalidDigestSize { expected: 32, actual: 16 }");
    }
}
