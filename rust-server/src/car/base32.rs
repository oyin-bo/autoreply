/// RFC 4648 base32 (lowercase) decoder for multibase 'b' prefix
/// No external dependencies; bit-level implementation
use crate::car::CarError;

#[allow(dead_code)]
const BASE32_ALPHABET: &[u8] = b"abcdefghijklmnopqrstuvwxyz234567";

#[allow(dead_code)]
fn char_to_value(c: u8) -> Option<u8> {
    match c {
        b'a'..=b'z' => Some(c - b'a'),
        b'2'..=b'7' => Some(26 + (c - b'2')),
        _ => None,
    }
}

/// Decode base32 (RFC 4648, lowercase) to bytes
/// Ignores padding ('=') and validates input
#[allow(dead_code)]
pub fn decode_base32(input: &str) -> Result<Vec<u8>, CarError> {
    let input = input.as_bytes();
    let mut output = Vec::with_capacity((input.len() * 5) / 8);

    let mut buffer: u64 = 0;
    let mut bits_in_buffer = 0;

    for &byte in input {
        if byte == b'=' {
            // Skip padding
            continue;
        }

        let value = char_to_value(byte).ok_or_else(|| {
            CarError::InvalidHeader(format!("Invalid base32 character: {}", byte as char))
        })?;

        buffer = (buffer << 5) | (value as u64);
        bits_in_buffer += 5;

        if bits_in_buffer >= 8 {
            bits_in_buffer -= 8;
            output.push((buffer >> bits_in_buffer) as u8);
            buffer &= (1 << bits_in_buffer) - 1;
        }
    }

    // If there are leftover bits, they should be zero padding
    if bits_in_buffer > 0 && buffer != 0 {
        return Err(CarError::InvalidHeader(
            "Invalid base32 padding".to_string(),
        ));
    }

    Ok(output)
}

/// Decode a multibase string (supports 'b' prefix for base32)
#[allow(dead_code)]
pub fn decode_multibase(input: &str) -> Result<Vec<u8>, CarError> {
    if input.is_empty() {
        return Err(CarError::InvalidHeader(
            "Empty multibase string".to_string(),
        ));
    }

    let prefix = input.chars().next().unwrap();
    let encoded = &input[prefix.len_utf8()..];

    match prefix {
        'b' => decode_base32(encoded),
        _ => Err(CarError::InvalidHeader(format!(
            "Unsupported multibase prefix: {}",
            prefix
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decode_base32_empty() {
        let result = decode_base32("");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), Vec::<u8>::new());
    }

    #[test]
    fn test_decode_base32_simple() {
        // "hello" -> "nbswy3dp"
        let result = decode_base32("nbswy3dp");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), b"hello");
    }

    #[test]
    fn test_decode_base32_with_padding() {
        // "hello" with padding -> "nbswy3dp"
        let result = decode_base32("nbswy3dp====");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), b"hello");
    }

    #[test]
    fn test_decode_base32_all_chars() {
        // Test that all valid base32 chars decode without error
        let result = decode_base32("abcdefghijklmnopqrstuvwxyz234567");
        assert!(result.is_ok());
    }

    #[test]
    fn test_decode_base32_invalid_char() {
        let result = decode_base32("abc!def");
        assert!(result.is_err());
    }

    #[test]
    fn test_decode_multibase_base32() {
        // "bhello" with multibase 'b' prefix
        let result = decode_multibase("bnbswy3dp");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), b"hello");
    }

    #[test]
    fn test_decode_multibase_unsupported() {
        let result = decode_multibase("zhello");
        assert!(result.is_err());
    }

    #[test]
    fn test_decode_multibase_empty() {
        let result = decode_multibase("");
        assert!(result.is_err());
    }

    #[test]
    fn test_decode_base32_bytes() {
        // Test binary data round-trip conceptually (we only test decode here)
        // Encoded: "mfrgg===" -> [0x61, 0x62, 0x63]
        let result = decode_base32("mfrgg");
        assert!(result.is_ok());
        let decoded = result.unwrap();
        assert_eq!(decoded, b"abc");
    }
}
