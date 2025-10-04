use crate::car::{CarEntry, CarError, CarHeader, Cid};

pub struct SyncByteReader<'a> {
    buf: &'a [u8],
    pos: usize,
}

impl<'a> SyncByteReader<'a> {
    pub fn new(buf: &'a [u8]) -> Self {
        Self { buf, pos: 0 }
    }

    pub fn pos(&self) -> usize {
        self.pos
    }

    pub fn remaining(&self) -> usize {
        self.buf.len().saturating_sub(self.pos)
    }

    pub fn upto(&self, size: usize) -> &[u8] {
        let end = (self.pos + size).min(self.buf.len());
        &self.buf[self.pos..end]
    }

    pub fn exactly(&mut self, size: usize, seek: bool) -> Result<&[u8], CarError> {
        if self.remaining() < size {
            return Err(CarError::UnexpectedEof);
        }

        let result = &self.buf[self.pos..self.pos + size];
        if seek {
            self.pos += size;
        }
        Ok(result)
    }

    pub fn seek(&mut self, size: usize) -> Result<(), CarError> {
        if self.remaining() < size {
            return Err(CarError::UnexpectedEof);
        }
        self.pos += size;
        Ok(())
    }
}

fn read_varint(reader: &mut SyncByteReader, max_size: usize) -> Result<u64, CarError> {
    let available = reader.remaining().min(max_size);
    if available == 0 {
        return Err(CarError::UnexpectedEof);
    }

    let buf = reader.upto(available);

    // Try to decode varint manually based on the LEB128 format
    let mut value = 0u64;
    let mut bytes_read = 0;

    for (i, &byte) in buf.iter().enumerate() {
        if i >= max_size {
            return Err(CarError::InvalidVarintSize);
        }

        value |= ((byte & 0x7F) as u64) << (i * 7);
        bytes_read = i + 1;

        if byte & 0x80 == 0 {
            break;
        }

        if i == 9 && byte & 0x80 != 0 {
            return Err(CarError::VarintError("Varint too long".to_string()));
        }
    }

    if bytes_read == 0 {
        return Err(CarError::UnexpectedEof);
    }

    reader.seek(bytes_read)?;
    Ok(value)
}

fn read_cid(reader: &mut SyncByteReader) -> Result<Cid, CarError> {
    // CID format in CAR files: [version][codec][digest_type][digest_size][digest...]
    // First, peek at the 4-byte header to determine total size
    let head = reader.exactly(4, false)?;

    let version = head[0];
    let codec = head[1];
    let digest_type = head[2];
    let digest_size = head[3] as usize;

    if version != 1 {
        return Err(CarError::InvalidCidVersion(version));
    }

    // Validate codec (DAG-CBOR or Raw)
    if codec != 0x71 && codec != 0x55 {
        return Err(CarError::InvalidCidCodec(codec));
    }

    // Validate digest type (SHA256)
    if digest_type != 0x12 {
        return Err(CarError::InvalidHeader(format!(
            "Invalid digest type: {:#x}",
            digest_type
        )));
    }

    // Allow both 32-byte and 0-byte digests (for compatibility)
    if digest_size != 32 && digest_size != 0 {
        return Err(CarError::InvalidDigestSize {
            expected: 32,
            actual: digest_size,
        });
    }

    // Read the full CID (header + digest)
    let total_size = 4 + digest_size;
    let cid_bytes = reader.exactly(total_size, true)?.to_vec();
    let digest = if digest_size > 0 {
        cid_bytes[4..4 + digest_size].to_vec()
    } else {
        vec![]
    };

    Ok(Cid {
        version,
        codec,
        digest_type,
        digest,
    })
}

fn read_header(reader: &mut SyncByteReader) -> Result<CarHeader, CarError> {
    // Read header length
    let header_len = read_varint(reader, 10)? as usize;

    // Read header CBOR data
    let header_bytes = reader.exactly(header_len, true)?;

    // Decode CBOR header
    let header_value: serde_cbor::Value = serde_cbor::from_slice(header_bytes)?;

    // Extract version and roots using pattern matching
    let header_map = match header_value {
        serde_cbor::Value::Map(map) => map,
        _ => return Err(CarError::InvalidHeader("Header is not a map".to_string())),
    };

    let version = header_map
        .get(&serde_cbor::Value::Text("version".to_string()))
        .and_then(|v| match v {
            serde_cbor::Value::Integer(i) => Some(*i as u8),
            _ => None,
        })
        .ok_or_else(|| CarError::InvalidHeader("Missing version".to_string()))?;

    if version != 1 {
        return Err(CarError::InvalidHeader(format!(
            "Unsupported version: {}",
            version
        )));
    }

    let roots_value = header_map
        .get(&serde_cbor::Value::Text("roots".to_string()))
        .ok_or_else(|| CarError::InvalidHeader("Missing roots".to_string()))?;

    let roots_array = match roots_value {
        serde_cbor::Value::Array(arr) => arr,
        _ => return Err(CarError::InvalidHeader("Roots is not an array".to_string())),
    };

    let mut roots = Vec::new();
    for root_value in roots_array {
        let root_bytes = match root_value {
            serde_cbor::Value::Bytes(bytes) => bytes,
            _ => return Err(CarError::InvalidHeader("Root CID is not bytes".to_string())),
        };

        // Parse root CID directly (no varint prefix in CAR header CIDs)
        let mut root_reader = SyncByteReader::new(root_bytes);

        let version = root_reader.exactly(1, true)?[0];
        let codec = root_reader.exactly(1, true)?[0];
        let digest_type = root_reader.exactly(1, true)?[0];
        let digest_size = root_reader.exactly(1, true)?[0] as usize;
        let digest = root_reader.exactly(digest_size, true)?.to_vec();

        roots.push(Cid {
            version,
            codec,
            digest_type,
            digest,
        });
    }

    Ok(CarHeader { version, roots })
}

pub struct SyncCarReader<'a> {
    reader: SyncByteReader<'a>,
    _header: CarHeader,
}

impl<'a> SyncCarReader<'a> {
    pub fn from_bytes(buf: &'a [u8]) -> Result<Self, CarError> {
        let mut reader = SyncByteReader::new(buf);
        let header = read_header(&mut reader)?;

        Ok(Self {
            reader,
            _header: header,
        })
    }
}

impl<'a> Iterator for SyncCarReader<'a> {
    type Item = Result<CarEntry, CarError>;

    fn next(&mut self) -> Option<Self::Item> {
        // Check if we have enough bytes for at least a small entry
        if self.reader.remaining() < 8 {
            return None;
        }

        // Read entry size
        let entry_size = match read_varint(&mut self.reader, 10) {
            Ok(size) => size as usize,
            Err(e) => return Some(Err(e)),
        };

        if self.reader.remaining() < entry_size {
            return Some(Err(CarError::UnexpectedEof));
        }

        let cid_start = self.reader.pos();

        // Read CID
        let cid = match read_cid(&mut self.reader) {
            Ok(cid) => cid,
            Err(e) => return Some(Err(e)),
        };

        let cid_end = self.reader.pos();
        let cid_size = cid_end - cid_start;

        // Calculate remaining bytes for data
        let bytes_size = entry_size - cid_size;

        let bytes = match self.reader.exactly(bytes_size, true) {
            Ok(bytes) => bytes.to_vec(),
            Err(e) => return Some(Err(e)),
        };

        Some(Ok(CarEntry { cid, bytes }))
    }
}

/// Iterator over AT Protocol records from a CAR file
/// Yields (record_type, cbor_data) tuples for efficient streaming processing
pub struct CarRecords {
    car_bytes: Vec<u8>,
    car_reader: Option<SyncCarReader<'static>>,
    processed_count: usize,
}

impl CarRecords {
    /// Create a new CarRecords iterator from owned CAR file bytes
    pub fn from_bytes(buf: Vec<u8>) -> Result<Self, CarError> {
        Ok(Self {
            car_bytes: buf,
            car_reader: None,
            processed_count: 0,
        })
    }

    /// Initialize the CAR reader on first use
    fn ensure_reader(&mut self) -> Result<(), CarError> {
        if self.car_reader.is_none() {
            // Create a CAR reader from our owned bytes
            // SAFETY: We own car_bytes for the lifetime of self, so this is safe
            let reader = unsafe {
                let bytes_ref: &'static [u8] = std::mem::transmute(self.car_bytes.as_slice());
                SyncCarReader::from_bytes(bytes_ref)?
            };
            self.car_reader = Some(reader);
        }
        Ok(())
    }
}

impl Iterator for CarRecords {
    type Item = Result<(String, Vec<u8>), CarError>;

    fn next(&mut self) -> Option<Self::Item> {
        // Initialize reader on first call
        if let Err(e) = self.ensure_reader() {
            return Some(Err(e));
        }

        // Stream through CAR entries one by one
        if let Some(ref mut reader) = self.car_reader {
            for entry_result in reader.by_ref() {
                let entry = match entry_result {
                    Ok(entry) => entry,
                    Err(e) => return Some(Err(e)),
                };

                self.processed_count += 1;

                // Try to decode CBOR and check if it's an AT Protocol record
                if let Ok(serde_cbor::Value::Map(ref cbor_map)) =
                    serde_cbor::from_slice::<serde_cbor::Value>(&entry.bytes)
                {
                    // Look for $type field to identify AT Protocol records
                    for (key, value) in cbor_map.iter() {
                        if let serde_cbor::Value::Text(key_str) = key {
                            if key_str == "$type" {
                                if let serde_cbor::Value::Text(type_str) = value {
                                    // Found an AT Protocol record - return owned data
                                    return Some(Ok((type_str.clone(), entry.bytes.clone())));
                                }
                            }
                        }
                    }
                }
                // This CAR block wasn't an AT Protocol record, continue to next
            }
        }

        // No more CAR blocks to process
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_cbor::Value;
    use std::collections::BTreeMap;

    // Helper function to create a valid CAR header
    fn create_car_header() -> Vec<u8> {
        let mut header_map = BTreeMap::new();
        header_map.insert(Value::Text("version".to_string()), Value::Integer(1));
        header_map.insert(
            Value::Text("roots".to_string()),
            Value::Array(vec![Value::Bytes(vec![
                1,    // version
                0x71, // codec (DAG-CBOR)
                0x12, // digest type (SHA256)
                32,   // digest size
                // 32 bytes of digest
                0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B, 0x0C, 0x0D, 0x0E,
                0x0F, 0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18, 0x19, 0x1A, 0x1B, 0x1C,
                0x1D, 0x1E, 0x1F, 0x20,
            ])]),
        );

        let header_cbor = serde_cbor::to_vec(&Value::Map(header_map)).unwrap();

        // Encode header length as varint + header
        let mut result = Vec::new();
        let header_len = header_cbor.len() as u64;

        // Simple varint encoding for small values
        if header_len < 128 {
            result.push(header_len as u8);
        } else {
            result.push((header_len & 0x7F) as u8 | 0x80);
            result.push((header_len >> 7) as u8);
        }

        result.extend_from_slice(&header_cbor);
        result
    }

    // Helper function to create a CAR entry with AT Protocol record
    fn create_at_protocol_entry(record_type: &str, text: &str) -> Vec<u8> {
        // Create CBOR record with $type field
        let mut record = BTreeMap::new();
        record.insert(
            Value::Text("$type".to_string()),
            Value::Text(record_type.to_string()),
        );
        record.insert(
            Value::Text("text".to_string()),
            Value::Text(text.to_string()),
        );

        let record_cbor = serde_cbor::to_vec(&Value::Map(record)).unwrap();

        // Create CID
        let cid_bytes = vec![
            1,    // version
            0x71, // codec (DAG-CBOR)
            0x12, // digest type (SHA256)
            32,   // digest size
            // 32 bytes of digest
            0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B, 0x0C, 0x0D, 0x0E,
            0x0F, 0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18, 0x19, 0x1A, 0x1B, 0x1C,
            0x1D, 0x1E, 0x1F, 0x20,
        ];

        // Entry size = CID size + CBOR size
        let entry_size = cid_bytes.len() + record_cbor.len();

        let mut result = Vec::new();

        // Encode entry size as varint
        if entry_size < 128 {
            result.push(entry_size as u8);
        } else {
            result.push((entry_size & 0x7F) as u8 | 0x80);
            result.push((entry_size >> 7) as u8);
        }

        // Add CID and CBOR data
        result.extend_from_slice(&cid_bytes);
        result.extend_from_slice(&record_cbor);

        result
    }

    // Helper function to create non-AT-Protocol entry
    fn create_non_at_protocol_entry() -> Vec<u8> {
        // Create CBOR record without $type field
        let mut record = BTreeMap::new();
        record.insert(
            Value::Text("data".to_string()),
            Value::Text("not an AT protocol record".to_string()),
        );

        let record_cbor = serde_cbor::to_vec(&Value::Map(record)).unwrap();

        // Create CID
        let cid_bytes = vec![
            1,    // version
            0x71, // codec (DAG-CBOR)
            0x12, // digest type (SHA256)
            32,   // digest size
            // 32 bytes of digest
            0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF, 0x00, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77,
            0x88, 0x99, 0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF, 0x00, 0x11, 0x22, 0x33, 0x44, 0x55,
            0x66, 0x77, 0x88, 0x99,
        ];

        let entry_size = cid_bytes.len() + record_cbor.len();

        let mut result = Vec::new();
        if entry_size < 128 {
            result.push(entry_size as u8);
        } else {
            result.push((entry_size & 0x7F) as u8 | 0x80);
            result.push((entry_size >> 7) as u8);
        }

        result.extend_from_slice(&cid_bytes);
        result.extend_from_slice(&record_cbor);

        result
    }

    #[test]
    fn test_sync_byte_reader_basic_operations() {
        let data = b"hello world";
        let mut reader = SyncByteReader::new(data);

        assert_eq!(reader.pos(), 0);
        assert_eq!(reader.remaining(), 11);

        let first_5 = reader.exactly(5, true).unwrap();
        assert_eq!(first_5, b"hello");
        assert_eq!(reader.pos(), 5);
        assert_eq!(reader.remaining(), 6);

        reader.seek(1).unwrap();
        assert_eq!(reader.pos(), 6);
        assert_eq!(reader.remaining(), 5);

        let rest = reader.exactly(5, true).unwrap();
        assert_eq!(rest, b"world");
        assert_eq!(reader.remaining(), 0);
    }

    #[test]
    fn test_sync_byte_reader_errors() {
        let data = b"short";
        let mut reader = SyncByteReader::new(data);

        // Test reading beyond buffer
        assert!(matches!(
            reader.exactly(10, true),
            Err(CarError::UnexpectedEof)
        ));

        // Test seeking beyond buffer
        assert!(matches!(reader.seek(10), Err(CarError::UnexpectedEof)));
    }

    #[test]
    fn test_read_varint() {
        let data = vec![42u8]; // Single byte varint
        let mut reader = SyncByteReader::new(&data);
        let result = read_varint(&mut reader, 10).unwrap();
        assert_eq!(result, 42);

        // Test multi-byte varint
        let data = vec![0x80 | 42, 1]; // Two-byte varint: 42 + (1 << 7) = 170
        let mut reader = SyncByteReader::new(&data);
        let result = read_varint(&mut reader, 10).unwrap();
        assert_eq!(result, 170);
    }

    #[test]
    fn test_read_varint_errors() {
        // Empty buffer
        let data = vec![];
        let mut reader = SyncByteReader::new(&data);
        assert!(matches!(
            read_varint(&mut reader, 10),
            Err(CarError::UnexpectedEof)
        ));

        // Varint too long (all bytes have continuation bit set)
        let data = vec![0xFF; 11];
        let mut reader = SyncByteReader::new(&data);
        assert!(matches!(
            read_varint(&mut reader, 10),
            Err(CarError::VarintError(_))
        ));
    }

    #[test]
    fn test_car_records_empty_iterator() {
        // Create minimal CAR with just header, no entries
        let car_data = create_car_header();

        let records = CarRecords::from_bytes(car_data).unwrap();
        let collected: Result<Vec<_>, _> = records.collect();
        let results = collected.unwrap();
        assert_eq!(results.len(), 0);
    }

    #[test]
    fn test_car_records_single_at_protocol_record() {
        let mut car_data = create_car_header();
        car_data.extend_from_slice(&create_at_protocol_entry(
            "app.bsky.feed.post",
            "Hello world!",
        ));

        let records = CarRecords::from_bytes(car_data).unwrap();
        let collected: Result<Vec<_>, _> = records.collect();
        let results = collected.unwrap();

        assert_eq!(results.len(), 1);
        let (record_type, cbor_data) = &results[0];
        assert_eq!(record_type, "app.bsky.feed.post");

        // Verify we can decode the CBOR data
        let decoded: Value = serde_cbor::from_slice(cbor_data).unwrap();
        if let Value::Map(map) = decoded {
            assert_eq!(
                map.get(&Value::Text("text".to_string())),
                Some(&Value::Text("Hello world!".to_string()))
            );
        } else {
            panic!("Expected CBOR map");
        }
    }

    #[test]
    fn test_car_records_multiple_records() {
        let mut car_data = create_car_header();
        car_data.extend_from_slice(&create_at_protocol_entry(
            "app.bsky.feed.post",
            "First post",
        ));
        car_data.extend_from_slice(&create_at_protocol_entry(
            "app.bsky.actor.profile",
            "My profile",
        ));
        car_data.extend_from_slice(&create_non_at_protocol_entry()); // Should be skipped
        car_data.extend_from_slice(&create_at_protocol_entry("app.bsky.feed.like", "Like this"));

        let records = CarRecords::from_bytes(car_data).unwrap();
        let collected: Result<Vec<_>, _> = records.collect();
        let results = collected.unwrap();

        assert_eq!(results.len(), 3); // Non-AT-Protocol entry should be filtered out

        let (type1, _) = &results[0];
        let (type2, _) = &results[1];
        let (type3, _) = &results[2];

        assert_eq!(type1, "app.bsky.feed.post");
        assert_eq!(type2, "app.bsky.actor.profile");
        assert_eq!(type3, "app.bsky.feed.like");
    }

    #[test]
    fn test_car_records_filters_non_at_protocol() {
        let mut car_data = create_car_header();
        car_data.extend_from_slice(&create_non_at_protocol_entry());
        car_data.extend_from_slice(&create_non_at_protocol_entry());

        let records = CarRecords::from_bytes(car_data).unwrap();
        let collected: Result<Vec<_>, _> = records.collect();
        let results = collected.unwrap();

        assert_eq!(results.len(), 0); // All entries should be filtered out
    }

    #[test]
    fn test_car_records_invalid_car_data() {
        // Test with invalid CAR data
        let invalid_data = vec![0xFF, 0xFF, 0xFF];
        let result = CarRecords::from_bytes(invalid_data);
        // Should fail during iteration, not creation
        assert!(result.is_ok());

        let records = result.unwrap();
        let collected: Result<Vec<_>, _> = records.collect();
        assert!(collected.is_err());
    }

    #[test]
    fn test_sync_car_reader_iterator() {
        let mut car_data = create_car_header();
        car_data.extend_from_slice(&create_at_protocol_entry("app.bsky.feed.post", "Test post"));

        let car_reader = SyncCarReader::from_bytes(&car_data).unwrap();
        let entries: Result<Vec<_>, _> = car_reader.collect();
        let results = entries.unwrap();

        assert_eq!(results.len(), 1);
        let entry = &results[0];

        // Verify CID structure
        assert_eq!(entry.cid.version, 1);
        assert_eq!(entry.cid.codec, 0x71);
        assert_eq!(entry.cid.digest_type, 0x12);
        assert_eq!(entry.cid.digest.len(), 32);

        // Verify CBOR data can be decoded
        let decoded: Value = serde_cbor::from_slice(&entry.bytes).unwrap();
        if let Value::Map(map) = decoded {
            assert_eq!(
                map.get(&Value::Text("$type".to_string())),
                Some(&Value::Text("app.bsky.feed.post".to_string()))
            );
        } else {
            panic!("Expected CBOR map");
        }
    }

    #[test]
    fn test_read_cid_validation() {
        // Test invalid version
        let mut invalid_version = vec![2, 0x71, 0x12, 32]; // version 2 is invalid
        invalid_version.extend(vec![0; 32]); // 32 bytes of digest
        let mut reader = SyncByteReader::new(&invalid_version);
        assert!(matches!(
            read_cid(&mut reader),
            Err(CarError::InvalidCidVersion(2))
        ));

        // Test invalid codec
        let mut invalid_codec = vec![1, 0x99, 0x12, 32]; // invalid codec
        invalid_codec.extend(vec![0; 32]);
        let mut reader = SyncByteReader::new(&invalid_codec);
        assert!(matches!(
            read_cid(&mut reader),
            Err(CarError::InvalidCidCodec(0x99))
        ));

        // Test invalid digest size
        let mut invalid_digest = vec![1, 0x71, 0x12, 16]; // wrong digest size
        invalid_digest.extend(vec![0; 16]);
        let mut reader = SyncByteReader::new(&invalid_digest);
        assert!(matches!(
            read_cid(&mut reader),
            Err(CarError::InvalidDigestSize {
                expected: 32,
                actual: 16
            })
        ));
    }
}
