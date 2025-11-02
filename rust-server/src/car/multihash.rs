/// Minimal multihash parser for CIDv1
/// Supports SHA-256 (0x12) with 32-byte digests
/// No external dependencies
use crate::car::CarError;

/// Parsed multihash components
#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq)]
pub struct Multihash {
    pub code: u8,
    pub size: usize,
    pub digest: Vec<u8>,
}

/// Parse a multihash from bytes
/// Format: [code_varint][size_varint][digest_bytes...]
/// We support only single-byte codes for simplicity (SHA-256 = 0x12)
#[allow(dead_code)]
pub fn parse_multihash(bytes: &[u8]) -> Result<Multihash, CarError> {
    if bytes.is_empty() {
        return Err(CarError::InvalidHeader("Empty multihash".to_string()));
    }

    let mut pos = 0;

    // Read hash code (we only support single-byte codes for now)
    let code = bytes[pos];
    pos += 1;

    if code != 0x12 {
        return Err(CarError::InvalidHeader(format!(
            "Unsupported multihash code: {:#x}",
            code
        )));
    }

    // Read digest size (we only support single-byte sizes for now)
    if pos >= bytes.len() {
        return Err(CarError::UnexpectedEof);
    }
    let size = bytes[pos] as usize;
    pos += 1;

    if size != 32 {
        return Err(CarError::InvalidDigestSize {
            expected: 32,
            actual: size,
        });
    }

    // Read digest bytes
    if pos + size > bytes.len() {
        return Err(CarError::UnexpectedEof);
    }
    let digest = bytes[pos..pos + size].to_vec();

    Ok(Multihash { code, size, digest })
}

/// Extract just the digest from a multihash (for use as canonical CID key)
#[allow(dead_code)]
pub fn extract_digest(bytes: &[u8]) -> Result<Vec<u8>, CarError> {
    let mh = parse_multihash(bytes)?;
    Ok(mh.digest)
}

/// Format digest as hex string (canonical CID key)
#[allow(dead_code)]
pub fn digest_to_hex(digest: &[u8]) -> String {
    hex::encode(digest)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_multihash_sha256() {
        let mut bytes = vec![0x12, 32]; // SHA-256, 32 bytes
        bytes.extend(vec![0xAB; 32]);

        let result = parse_multihash(&bytes);
        assert!(result.is_ok());

        let mh = result.unwrap();
        assert_eq!(mh.code, 0x12);
        assert_eq!(mh.size, 32);
        assert_eq!(mh.digest.len(), 32);
        assert_eq!(mh.digest[0], 0xAB);
    }

    #[test]
    fn test_parse_multihash_invalid_code() {
        let bytes = vec![0x13, 32]; // Unsupported code
        let result = parse_multihash(&bytes);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_multihash_invalid_size() {
        let mut bytes = vec![0x12, 16]; // Wrong size
        bytes.extend(vec![0; 16]);
        let result = parse_multihash(&bytes);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_multihash_truncated() {
        let bytes = vec![0x12, 32, 0x01, 0x02]; // Only 2 bytes instead of 32
        let result = parse_multihash(&bytes);
        assert!(result.is_err());
    }

    #[test]
    fn test_extract_digest() {
        let mut bytes = vec![0x12, 32];
        let expected_digest = vec![0x01, 0x02, 0x03, 0x04];
        bytes.extend(&expected_digest);
        bytes.extend(vec![0; 28]); // Pad to 32 bytes

        let result = extract_digest(&bytes);
        assert!(result.is_ok());

        let digest = result.unwrap();
        assert_eq!(digest.len(), 32);
        assert_eq!(&digest[0..4], &expected_digest[..]);
    }

    #[test]
    fn test_digest_to_hex() {
        let digest = vec![0x01, 0x23, 0x45, 0x67, 0x89, 0xAB, 0xCD, 0xEF];
        let hex = digest_to_hex(&digest);
        assert_eq!(hex, "0123456789abcdef");
    }

    #[test]
    fn test_parse_multihash_empty() {
        let result = parse_multihash(&[]);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_multihash_only_code() {
        let result = parse_multihash(&[0x12]);
        assert!(result.is_err());
    }
}
