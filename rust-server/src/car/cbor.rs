/// Minimal DAG-CBOR decoder for AT Protocol records
/// Supports: maps, arrays, text, bytes, integers, booleans, null, tag-0 (CID links)
/// Zero-copy where possible; no external CBOR libraries
use crate::car::CarError;

#[derive(Debug, Clone, PartialEq)]
pub enum CborValue<'a> {
    Map(Vec<(CborValue<'a>, CborValue<'a>)>),
    Array(Vec<CborValue<'a>>),
    Text(&'a str),
    Bytes(&'a [u8]),
    Integer(i64),
    Bool(bool),
    Null,
    /// Tag 42 with CID bytes (for DAG-CBOR links)
    Link(&'a [u8]),
}

pub struct CborReader<'a> {
    buf: &'a [u8],
    pos: usize,
}

impl<'a> CborReader<'a> {
    pub fn new(buf: &'a [u8]) -> Self {
        Self { buf, pos: 0 }
    }

    fn remaining(&self) -> usize {
        self.buf.len().saturating_sub(self.pos)
    }

    fn read_byte(&mut self) -> Result<u8, CarError> {
        if self.pos >= self.buf.len() {
            return Err(CarError::UnexpectedEof);
        }
        let byte = self.buf[self.pos];
        self.pos += 1;
        Ok(byte)
    }

    fn read_bytes(&mut self, n: usize) -> Result<&'a [u8], CarError> {
        if self.remaining() < n {
            return Err(CarError::UnexpectedEof);
        }
        let bytes = &self.buf[self.pos..self.pos + n];
        self.pos += n;
        Ok(bytes)
    }

    fn read_uint(&mut self, additional: u8) -> Result<u64, CarError> {
        match additional {
            0..=23 => Ok(additional as u64),
            24 => Ok(self.read_byte()? as u64),
            25 => {
                let bytes = self.read_bytes(2)?;
                Ok(u16::from_be_bytes([bytes[0], bytes[1]]) as u64)
            }
            26 => {
                let bytes = self.read_bytes(4)?;
                Ok(u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) as u64)
            }
            27 => {
                let bytes = self.read_bytes(8)?;
                Ok(u64::from_be_bytes([
                    bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
                ]))
            }
            _ => Err(CarError::InvalidHeader(
                "Invalid additional info in CBOR".to_string(),
            )),
        }
    }

    pub fn read_value(&mut self) -> Result<CborValue<'a>, CarError> {
        let initial = self.read_byte()?;
        let major = initial >> 5;
        let additional = initial & 0x1F;

        match major {
            0 => {
                // Unsigned integer
                let val = self.read_uint(additional)?;
                Ok(CborValue::Integer(val as i64))
            }
            1 => {
                // Negative integer
                let val = self.read_uint(additional)?;
                Ok(CborValue::Integer(-1 - (val as i64)))
            }
            2 => {
                // Byte string
                let len = self.read_uint(additional)? as usize;
                let bytes = self.read_bytes(len)?;
                Ok(CborValue::Bytes(bytes))
            }
            3 => {
                // Text string
                let len = self.read_uint(additional)? as usize;
                let bytes = self.read_bytes(len)?;
                let text = std::str::from_utf8(bytes).map_err(CarError::Utf8StrError)?;
                Ok(CborValue::Text(text))
            }
            4 => {
                // Array
                let len = self.read_uint(additional)? as usize;
                let mut items = Vec::with_capacity(len);
                for _ in 0..len {
                    items.push(self.read_value()?);
                }
                Ok(CborValue::Array(items))
            }
            5 => {
                // Map
                let len = self.read_uint(additional)? as usize;
                let mut pairs = Vec::with_capacity(len);
                for _ in 0..len {
                    let key = self.read_value()?;
                    let value = self.read_value()?;
                    pairs.push((key, value));
                }
                Ok(CborValue::Map(pairs))
            }
            6 => {
                // Tag
                let tag = self.read_uint(additional)?;
                if tag == 42 {
                    // DAG-CBOR CID link (tag 42)
                    // The value should be a byte string containing the CID
                    let value = self.read_value()?;
                    match value {
                        CborValue::Bytes(bytes) => {
                            // CID bytes: should start with 0x00 for CIDv1
                            if bytes.is_empty() {
                                return Err(CarError::InvalidHeader("Empty CID link".to_string()));
                            }
                            // Skip the leading 0x00 marker if present
                            let cid_bytes = if bytes[0] == 0x00 { &bytes[1..] } else { bytes };
                            Ok(CborValue::Link(cid_bytes))
                        }
                        _ => Err(CarError::InvalidHeader(
                            "CID link must be bytes".to_string(),
                        )),
                    }
                } else {
                    // Skip other tags by reading and discarding the value
                    let _value = self.read_value()?;
                    Err(CarError::InvalidHeader(format!("Unsupported tag: {}", tag)))
                }
            }
            7 => {
                // Simple values
                match additional {
                    20 => Ok(CborValue::Bool(false)),
                    21 => Ok(CborValue::Bool(true)),
                    22 => Ok(CborValue::Null),
                    _ => Err(CarError::InvalidHeader(
                        "Unsupported CBOR simple value".to_string(),
                    )),
                }
            }
            _ => Err(CarError::InvalidHeader(
                "Invalid CBOR major type".to_string(),
            )),
        }
    }
}

/// Decode CBOR bytes to CborValue
pub fn decode_cbor(bytes: &[u8]) -> Result<CborValue<'_>, CarError> {
    let mut reader = CborReader::new(bytes);
    reader.read_value()
}

/// Helper to extract a string field from a CBOR map
pub fn get_text_field<'a>(map: &'a [(CborValue<'a>, CborValue<'a>)], key: &str) -> Option<&'a str> {
    for (k, v) in map {
        if let CborValue::Text(k_str) = k {
            if *k_str == key {
                if let CborValue::Text(v_str) = v {
                    return Some(v_str);
                }
            }
        }
    }
    None
}

/// Helper to extract an array field from a CBOR map
pub fn get_array_field<'a>(map: &'a [(CborValue<'a>, CborValue<'a>)], key: &str) -> Option<&'a [CborValue<'a>]> {
    for (k, v) in map {
        if let CborValue::Text(k_str) = k {
            if *k_str == key {
                if let CborValue::Array(arr) = v {
                    return Some(arr.as_slice());
                }
            }
        }
    }
    None
}

/// Helper to extract a map field from a CBOR map
pub fn get_map_field<'a>(map: &'a [(CborValue<'a>, CborValue<'a>)], key: &str) -> Option<&'a [(CborValue<'a>, CborValue<'a>)]> {
    for (k, v) in map {
        if let CborValue::Text(k_str) = k {
            if *k_str == key {
                if let CborValue::Map(m) = v {
                    return Some(m.as_slice());
                }
            }
        }
    }
    None
}

/// Helper to extract an integer field from a CBOR map
pub fn get_int_field(map: &[(CborValue<'_>, CborValue<'_>)], key: &str) -> Option<i64> {
    for (k, v) in map {
        if let CborValue::Text(k_str) = k {
            if *k_str == key {
                if let CborValue::Integer(i) = v {
                    return Some(*i);
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decode_integer() {
        // CBOR encoding of integer 42: 0x18, 0x2A
        let bytes = [0x18, 0x2A];
        let result = decode_cbor(&bytes);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), CborValue::Integer(42));
    }

    #[test]
    fn test_decode_small_integer() {
        // CBOR encoding of integer 5: 0x05
        let bytes = [0x05];
        let result = decode_cbor(&bytes);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), CborValue::Integer(5));
    }

    #[test]
    fn test_decode_negative_integer() {
        // CBOR encoding of -1: 0x20
        let bytes = [0x20];
        let result = decode_cbor(&bytes);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), CborValue::Integer(-1));
    }

    #[test]
    fn test_decode_text() {
        // CBOR encoding of "hello": 0x65 (text, length 5), then "hello"
        let bytes = [0x65, b'h', b'e', b'l', b'l', b'o'];
        let result = decode_cbor(&bytes);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), CborValue::Text("hello"));
    }

    #[test]
    fn test_decode_bytes() {
        // CBOR encoding of bytes [1,2,3]: 0x43 (bytes, length 3), then bytes
        let bytes = [0x43, 0x01, 0x02, 0x03];
        let result = decode_cbor(&bytes);
        assert!(result.is_ok());
        match result.unwrap() {
            CborValue::Bytes(b) => assert_eq!(b, &[0x01, 0x02, 0x03]),
            _ => panic!("Expected bytes"),
        }
    }

    #[test]
    fn test_decode_bool_true() {
        // CBOR encoding of true: 0xF5
        let bytes = [0xF5];
        let result = decode_cbor(&bytes);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), CborValue::Bool(true));
    }

    #[test]
    fn test_decode_bool_false() {
        // CBOR encoding of false: 0xF4
        let bytes = [0xF4];
        let result = decode_cbor(&bytes);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), CborValue::Bool(false));
    }

    #[test]
    fn test_decode_null() {
        // CBOR encoding of null: 0xF6
        let bytes = [0xF6];
        let result = decode_cbor(&bytes);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), CborValue::Null);
    }

    #[test]
    fn test_decode_array() {
        // CBOR encoding of [1, 2, 3]: 0x83 (array, length 3), then 1, 2, 3
        let bytes = [0x83, 0x01, 0x02, 0x03];
        let result = decode_cbor(&bytes);
        assert!(result.is_ok());
        match result.unwrap() {
            CborValue::Array(arr) => {
                assert_eq!(arr.len(), 3);
                assert_eq!(arr[0], CborValue::Integer(1));
                assert_eq!(arr[1], CborValue::Integer(2));
                assert_eq!(arr[2], CborValue::Integer(3));
            }
            _ => panic!("Expected array"),
        }
    }

    #[test]
    fn test_decode_map() {
        // CBOR encoding of {"a": 1}: 0xA1 (map, 1 pair), "a" (0x61, 0x61), 1 (0x01)
        let bytes = [0xA1, 0x61, b'a', 0x01];
        let result = decode_cbor(&bytes);
        assert!(result.is_ok());
        match result.unwrap() {
            CborValue::Map(map) => {
                assert_eq!(map.len(), 1);
                assert_eq!(map[0].0, CborValue::Text("a"));
                assert_eq!(map[0].1, CborValue::Integer(1));
            }
            _ => panic!("Expected map"),
        }
    }

    #[test]
    fn test_decode_nested_map() {
        // {"type": "post", "text": "hi"}
        // Map with 2 entries: 0xA2
        // "type": 0x64 't' 'y' 'p' 'e'
        // "post": 0x64 'p' 'o' 's' 't'
        // "text": 0x64 't' 'e' 'x' 't'
        // "hi": 0x62 'h' 'i'
        let bytes = [
            0xA2, // map(2)
            0x64, b't', b'y', b'p', b'e', // "type"
            0x64, b'p', b'o', b's', b't', // "post"
            0x64, b't', b'e', b'x', b't', // "text"
            0x62, b'h', b'i', // "hi"
        ];
        let result = decode_cbor(&bytes);
        assert!(result.is_ok());
        match result.unwrap() {
            CborValue::Map(map) => {
                assert_eq!(map.len(), 2);
                let type_val = get_text_field(&map, "type");
                assert_eq!(type_val, Some("post"));
                let text_val = get_text_field(&map, "text");
                assert_eq!(text_val, Some("hi"));
            }
            _ => panic!("Expected map"),
        }
    }

    #[test]
    fn test_decode_empty_array() {
        // CBOR encoding of []: 0x80
        let bytes = [0x80];
        let result = decode_cbor(&bytes);
        assert!(result.is_ok());
        match result.unwrap() {
            CborValue::Array(arr) => assert_eq!(arr.len(), 0),
            _ => panic!("Expected array"),
        }
    }

    #[test]
    fn test_decode_empty_map() {
        // CBOR encoding of {}: 0xA0
        let bytes = [0xA0];
        let result = decode_cbor(&bytes);
        assert!(result.is_ok());
        match result.unwrap() {
            CborValue::Map(map) => assert_eq!(map.len(), 0),
            _ => panic!("Expected map"),
        }
    }

    #[test]
    fn test_decode_truncated() {
        // Incomplete CBOR data
        let bytes = [0x65, b'h', b'e']; // Says 5 bytes but only has 2
        let result = decode_cbor(&bytes);
        assert!(result.is_err());
    }

    #[test]
    fn test_get_text_field() {
        let map = vec![
            (CborValue::Text("name"), CborValue::Text("Alice")),
            (CborValue::Text("age"), CborValue::Integer(30)),
        ];

        assert_eq!(get_text_field(&map, "name"), Some("Alice"));
        assert_eq!(get_text_field(&map, "age"), None); // Not a text value
        assert_eq!(get_text_field(&map, "missing"), None);
    }
}
