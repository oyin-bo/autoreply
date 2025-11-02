use crate::car::cbor::{decode_cbor, get_text_field, CborValue};
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
            // cap at 10 bytes for u64
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
    // Simplified in-file CID encoding for tests:
    // [version][codec][digest_type][digest_size][digest_bytes]
    let head = reader.exactly(4, false)?;
    let version = head[0];
    let codec = head[1];
    let digest_type = head[2];
    let digest_size = head[3] as usize;

    if version != 1 {
        return Err(CarError::InvalidCidVersion(version));
    }
    // Allow dag-cbor 0x71 and raw 0x55 for tests
    if codec != 0x71 && codec != 0x55 {
        return Err(CarError::InvalidCidCodec(codec));
    }
    // SHA-256 multihash code is 0x12; keep simple enforcement here
    if digest_type != 0x12 {
        return Err(CarError::InvalidHeader(format!(
            "Invalid digest type: {:#x}",
            digest_type
        )));
    }
    if digest_size != 32 && digest_size != 0 {
        return Err(CarError::InvalidDigestSize {
            expected: 32,
            actual: digest_size,
        });
    }

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
    // Read header length as varint, then parse CBOR map with version and roots
    let header_len = read_varint(reader, 10)? as usize;
    let header_bytes = reader.exactly(header_len, true)?;

    let header_value = decode_cbor(header_bytes)?;
    let header_map = match header_value {
        CborValue::Map(map) => map,
        _ => return Err(CarError::InvalidHeader("Header is not a map".to_string())),
    };

    let version = get_text_field(&header_map, "version")
        .and_then(|s| s.parse::<u8>().ok())
        .or_else(|| {
            // Try as integer
            for (k, v) in &header_map {
                if let CborValue::Text(key) = k {
                    if *key == "version" {
                        if let CborValue::Integer(i) = v {
                            return Some(*i as u8);
                        }
                    }
                }
            }
            None
        })
        .ok_or_else(|| CarError::InvalidHeader("Missing version".to_string()))?;

    if version != 1 {
        return Err(CarError::InvalidHeader(format!(
            "Unsupported version: {}",
            version
        )));
    }

    // Extract roots array
    let roots_value = header_map
        .iter()
        .find(|(k, _)| matches!(k, CborValue::Text(s) if *s == "roots"))
        .map(|(_, v)| v)
        .ok_or_else(|| CarError::InvalidHeader("Missing roots".to_string()))?;

    let roots_array = match roots_value {
        CborValue::Array(arr) => arr,
        _ => return Err(CarError::InvalidHeader("Roots is not an array".to_string())),
    };

    let mut roots = Vec::new();
    for root_value in roots_array {
        // Root CIDs can be encoded as either:
        // 1. Plain bytes (our manual test encoding)
        // 2. CBOR link (tag 42, actual AT Protocol CAR files)
        let root_bytes = match root_value {
            CborValue::Bytes(bytes) => bytes,
            CborValue::Link(cid_bytes) => cid_bytes,
            _ => {
                return Err(CarError::InvalidHeader(
                    "Root CID must be bytes or link".to_string(),
                ))
            }
        };

        let mut root_reader = SyncByteReader::new(root_bytes);
        // Some encodings may include a leading 0 marker; support both forms
        let first = root_reader.exactly(1, true)?[0];
        let version = if first == 0 {
            read_varint(&mut root_reader, 10)? as u8
        } else {
            first
        };
        let codec = read_varint(&mut root_reader, 10)? as u8;
        let digest_type = read_varint(&mut root_reader, 10)? as u8;
        let digest_size = read_varint(&mut root_reader, 10)? as usize;
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
    pub fn header(&self) -> &CarHeader {
        &self._header
    }
}

impl<'a> Iterator for SyncCarReader<'a> {
    type Item = Result<CarEntry, CarError>;
    fn next(&mut self) -> Option<Self::Item> {
        if self.reader.remaining() < 1 {
            return None;
        }

        // Entry size as varint
        let entry_size = match read_varint(&mut self.reader, 10) {
            Ok(size) => size as usize,
            Err(e) => return Some(Err(e)),
        };
        if self.reader.remaining() < entry_size {
            return Some(Err(CarError::UnexpectedEof));
        }

        // CID
        let cid_start = self.reader.pos();
        let cid = match read_cid(&mut self.reader) {
            Ok(cid) => cid,
            Err(e) => return Some(Err(e)),
        };
        let cid_end = self.reader.pos();
        let cid_size = cid_end - cid_start;

        // Block bytes
        let bytes_size = entry_size - cid_size;
        let bytes = match self.reader.exactly(bytes_size, true) {
            Ok(bytes) => bytes.to_vec(),
            Err(e) => return Some(Err(e)),
        };

        Some(Ok(CarEntry { cid, bytes }))
    }
}

pub struct CarRecords {
    car_bytes: Vec<u8>,
    car_reader: Option<SyncCarReader<'static>>, // internal self-referential lifetime via transmute
    processed_count: usize,
}

impl CarRecords {
    pub fn from_bytes(buf: Vec<u8>) -> Result<Self, CarError> {
        Ok(Self {
            car_bytes: buf,
            car_reader: None,
            processed_count: 0,
        })
    }

    fn ensure_reader(&mut self) -> Result<(), CarError> {
        if self.car_reader.is_none() {
            // SAFETY: we own car_bytes and store the reader in self, so the reference remains valid
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
    type Item = Result<(String, Vec<u8>, String), CarError>;

    fn next(&mut self) -> Option<Self::Item> {
        if let Err(e) = self.ensure_reader() {
            return Some(Err(e));
        }

        let reader = self.car_reader.as_mut()?;

        // Iterate underlying entries until we find an AT Protocol record or exhaust
        for entry_result in reader.by_ref() {
            let entry = match entry_result {
                Ok(e) => e,
                Err(e) => return Some(Err(e)),
            };
            self.processed_count += 1;
            let cid_str = format_cid_simple(&entry.cid);

            // Decode CBOR to find $type field
            if let Ok(CborValue::Map(ref cbor_map)) = decode_cbor(&entry.bytes) {
                if let Some(type_str) = get_text_field(cbor_map, "$type") {
                    return Some(Ok((type_str.to_string(), entry.bytes.clone(), cid_str)));
                }
            }
        }
        None
    }
}

fn format_cid_simple(cid: &Cid) -> String {
    format!(
        "v{}-c{:02x}-d{:02x}-{}",
        cid.version,
        cid.codec,
        cid.digest_type,
        hex::encode(&cid.digest)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    // Manual CBOR encoding helpers (no serde_cbor dependency)
    fn encode_cbor_map(pairs: &[(&str, CborVal)]) -> Vec<u8> {
        let mut result = vec![0xA0 | (pairs.len() as u8)]; // Map major type + length
        for (key, value) in pairs {
            result.extend(encode_cbor_text(key));
            result.extend(encode_cbor_val(value));
        }
        result
    }

    fn encode_cbor_text(s: &str) -> Vec<u8> {
        let bytes = s.as_bytes();
        let mut result = vec![];
        if bytes.len() <= 23 {
            result.push(0x60 | (bytes.len() as u8));
        } else if bytes.len() <= 255 {
            result.push(0x78);
            result.push(bytes.len() as u8);
        } else {
            panic!("Text too long for test helper");
        }
        result.extend_from_slice(bytes);
        result
    }

    fn encode_cbor_bytes(b: &[u8]) -> Vec<u8> {
        let mut result = vec![];
        if b.len() <= 23 {
            result.push(0x40 | (b.len() as u8));
        } else if b.len() <= 255 {
            result.push(0x58);
            result.push(b.len() as u8);
        } else {
            panic!("Bytes too long for test helper");
        }
        result.extend_from_slice(b);
        result
    }

    fn encode_cbor_int(i: i64) -> Vec<u8> {
        if i >= 0 {
            if i <= 23 {
                vec![i as u8]
            } else if i <= 255 {
                vec![0x18, i as u8]
            } else {
                panic!("Integer too large for test helper");
            }
        } else {
            let val = (-1 - i) as u64;
            if val <= 23 {
                vec![0x20 | (val as u8)]
            } else {
                panic!("Negative integer too large for test helper");
            }
        }
    }

    fn encode_cbor_array(items: &[CborVal]) -> Vec<u8> {
        let mut result = vec![0x80 | (items.len() as u8)];
        for item in items {
            result.extend(encode_cbor_val(item));
        }
        result
    }

    #[derive(Clone)]
    enum CborVal {
        Text(String),
        Int(i64),
        Bytes(Vec<u8>),
        Array(Vec<CborVal>),
    }

    fn encode_cbor_val(v: &CborVal) -> Vec<u8> {
        match v {
            CborVal::Text(s) => encode_cbor_text(s),
            CborVal::Int(i) => encode_cbor_int(*i),
            CborVal::Bytes(b) => encode_cbor_bytes(b),
            CborVal::Array(items) => encode_cbor_array(items),
        }
    }

    fn create_car_header() -> Vec<u8> {
        // CAR header: {version: 1, roots: [<cid_bytes>]}
        let cid_bytes = vec![
            1, 0x71, 0x12, 32, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B,
            0x0C, 0x0D, 0x0E, 0x0F, 0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18, 0x19,
            0x1A, 0x1B, 0x1C, 0x1D, 0x1E, 0x1F, 0x20,
        ];

        let header_cbor = encode_cbor_map(&[
            ("version", CborVal::Int(1)),
            ("roots", CborVal::Array(vec![CborVal::Bytes(cid_bytes)])),
        ]);

        let mut result = Vec::new();
        let header_len = header_cbor.len() as u64;
        if header_len < 128 {
            result.push(header_len as u8);
        } else {
            result.push((header_len & 0x7F) as u8 | 0x80);
            result.push((header_len >> 7) as u8);
        }
        result.extend_from_slice(&header_cbor);
        result
    }

    fn create_at_protocol_entry(record_type: &str, text: &str) -> Vec<u8> {
        let record_cbor = encode_cbor_map(&[
            ("$type", CborVal::Text(record_type.to_string())),
            ("text", CborVal::Text(text.to_string())),
        ]);

        let cid_bytes = vec![
            1, 0x71, 0x12, 32, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B,
            0x0C, 0x0D, 0x0E, 0x0F, 0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18, 0x19,
            0x1A, 0x1B, 0x1C, 0x1D, 0x1E, 0x1F, 0x20,
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

    fn create_non_at_protocol_entry() -> Vec<u8> {
        let record_cbor = encode_cbor_map(&[(
            "data",
            CborVal::Text("not an AT protocol record".to_string()),
        )]);

        let cid_bytes = vec![
            1, 0x71, 0x12, 32, 0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF, 0x00, 0x11, 0x22, 0x33, 0x44,
            0x55, 0x66, 0x77, 0x88, 0x99, 0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF, 0x00, 0x11, 0x22,
            0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99,
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
        assert!(matches!(
            reader.exactly(10, true),
            Err(CarError::UnexpectedEof)
        ));
        assert!(matches!(reader.seek(10), Err(CarError::UnexpectedEof)));
    }

    #[test]
    fn test_read_varint() {
        let data = vec![42u8];
        let mut reader = SyncByteReader::new(&data);
        let result = read_varint(&mut reader, 10).unwrap();
        assert_eq!(result, 42);

        let data = vec![0x80 | 42, 1];
        let mut reader = SyncByteReader::new(&data);
        let result = read_varint(&mut reader, 10).unwrap();
        assert_eq!(result, 170);
    }

    #[test]
    fn test_read_varint_errors() {
        let data = vec![];
        let mut reader = SyncByteReader::new(&data);
        assert!(matches!(
            read_varint(&mut reader, 10),
            Err(CarError::UnexpectedEof)
        ));

        let data = vec![0xFF; 11];
        let mut reader = SyncByteReader::new(&data);
        assert!(matches!(
            read_varint(&mut reader, 10),
            Err(CarError::VarintError(_))
        ));
    }

    #[test]
    fn test_car_records_empty_iterator() {
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
        let (record_type, cbor_data, _cid) = &results[0];
        assert_eq!(record_type, "app.bsky.feed.post");
        let decoded = decode_cbor(cbor_data).unwrap();
        if let CborValue::Map(map) = decoded {
            assert_eq!(get_text_field(&map, "text"), Some("Hello world!"));
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
        car_data.extend_from_slice(&create_non_at_protocol_entry());
        car_data.extend_from_slice(&create_at_protocol_entry("app.bsky.feed.like", "Like this"));
        let records = CarRecords::from_bytes(car_data).unwrap();
        let collected: Result<Vec<_>, _> = records.collect();
        let results = collected.unwrap();
        assert_eq!(results.len(), 3);
        let (type1, _, _) = &results[0];
        let (type2, _, _) = &results[1];
        let (type3, _, _) = &results[2];
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
        assert_eq!(results.len(), 0);
    }

    #[test]
    fn test_car_records_invalid_car_data() {
        // Starts with invalid header length varint and junk data, ensure error surfaces during iteration
        let invalid_data = vec![0xFF, 0xFF, 0xFF];
        let result = CarRecords::from_bytes(invalid_data);
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
        assert_eq!(entry.cid.version, 1);
        assert_eq!(entry.cid.codec, 0x71);
        assert_eq!(entry.cid.digest_type, 0x12);
        assert_eq!(entry.cid.digest.len(), 32);
        let decoded = decode_cbor(&entry.bytes).unwrap();
        if let CborValue::Map(map) = decoded {
            assert_eq!(get_text_field(&map, "$type"), Some("app.bsky.feed.post"));
        } else {
            panic!("Expected CBOR map");
        }
    }

    #[test]
    fn test_read_cid_validation() {
        let mut invalid_version = vec![2, 0x71, 0x12, 32];
        invalid_version.extend(vec![0; 32]);
        let mut reader = SyncByteReader::new(&invalid_version);
        assert!(matches!(
            read_cid(&mut reader),
            Err(CarError::InvalidCidVersion(2))
        ));

        let mut invalid_codec = vec![1, 0x99, 0x12, 32];
        invalid_codec.extend(vec![0; 32]);
        let mut reader = SyncByteReader::new(&invalid_codec);
        assert!(matches!(
            read_cid(&mut reader),
            Err(CarError::InvalidCidCodec(0x99))
        ));

        let mut invalid_digest = vec![1, 0x71, 0x12, 16];
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

    #[test]
    fn test_sync_byte_reader_upto() {
        let data = b"hello world";
        let reader = SyncByteReader::new(data);
        let first_5 = reader.upto(5);
        assert_eq!(first_5, b"hello");
        let beyond = reader.upto(100);
        assert_eq!(beyond, b"hello world");
        let zero = reader.upto(0);
        assert_eq!(zero, b"");
    }

    #[test]
    fn test_sync_byte_reader_exactly_without_seek() {
        let data = b"hello world";
        let mut reader = SyncByteReader::new(data);
        let first_5 = reader.exactly(5, false).unwrap();
        assert_eq!(first_5, b"hello");
        assert_eq!(reader.pos(), 0);
        let first_5_again = reader.exactly(5, true).unwrap();
        assert_eq!(first_5_again, b"hello");
        assert_eq!(reader.pos(), 5);
    }

    #[test]
    fn test_read_varint_boundary_cases() {
        let data = vec![127u8];
        let mut reader = SyncByteReader::new(&data);
        assert_eq!(read_varint(&mut reader, 10).unwrap(), 127);

        let data = vec![0x80, 0x01];
        let mut reader = SyncByteReader::new(&data);
        assert_eq!(read_varint(&mut reader, 10).unwrap(), 128);

        let data = vec![0x80, 0x80, 0x01];
        let mut reader = SyncByteReader::new(&data);
        assert_eq!(read_varint(&mut reader, 10).unwrap(), 16384);
    }

    #[test]
    fn test_read_header_invalid_structure() {
        // Manually construct CBOR header with version 2
        let header_cbor = encode_cbor_map(&[
            ("version", CborVal::Int(2)),
            ("roots", CborVal::Array(vec![])),
        ]);
        let mut data = Vec::new();
        data.push(header_cbor.len() as u8);
        data.extend_from_slice(&header_cbor);
        let mut reader = SyncByteReader::new(&data);
        let result = read_header(&mut reader);
        assert!(matches!(result, Err(CarError::InvalidHeader(_))));
    }

    #[test]
    fn test_read_header_missing_fields() {
        // Manually construct CBOR header missing roots field
        let header_cbor = encode_cbor_map(&[("version", CborVal::Int(1))]);
        let mut data = Vec::new();
        data.push(header_cbor.len() as u8);
        data.extend_from_slice(&header_cbor);
        let mut reader = SyncByteReader::new(&data);
        let result = read_header(&mut reader);
        assert!(matches!(result, Err(CarError::InvalidHeader(_))));
    }

    #[test]
    fn test_car_records_cid_formatting() {
        let mut car_data = create_car_header();
        car_data.extend_from_slice(&create_at_protocol_entry("app.bsky.feed.post", "Test CID"));
        let records = CarRecords::from_bytes(car_data).unwrap();
        let collected: Result<Vec<_>, _> = records.collect();
        let results = collected.unwrap();
        assert_eq!(results.len(), 1);
        let (_type, _cbor, cid) = &results[0];
        assert!(cid.starts_with("v1-"));
        assert!(cid.contains("-c71-"));
        assert!(cid.contains("-d12-"));
    }

    #[test]
    fn test_car_records_mixed_content() {
        let mut car_data = create_car_header();
        car_data.extend_from_slice(&create_at_protocol_entry("app.bsky.feed.post", "A post"));
        car_data.extend_from_slice(&create_at_protocol_entry(
            "app.bsky.feed.repost",
            "A repost",
        ));
        car_data.extend_from_slice(&create_at_protocol_entry(
            "app.bsky.graph.follow",
            "A follow",
        ));
        car_data.extend_from_slice(&create_at_protocol_entry("app.bsky.feed.like", "A like"));
        car_data.extend_from_slice(&create_non_at_protocol_entry());
        let records = CarRecords::from_bytes(car_data).unwrap();
        let collected: Result<Vec<_>, _> = records.collect();
        let results = collected.unwrap();
        assert_eq!(results.len(), 4);
        let types: Vec<_> = results.iter().map(|(t, _, _)| t.as_str()).collect();
        assert!(types.contains(&"app.bsky.feed.post"));
        assert!(types.contains(&"app.bsky.feed.repost"));
        assert!(types.contains(&"app.bsky.graph.follow"));
        assert!(types.contains(&"app.bsky.feed.like"));
    }

    #[test]
    fn test_car_records_cbor_decoding() {
        let mut car_data = create_car_header();
        car_data.extend_from_slice(&create_at_protocol_entry(
            "app.bsky.feed.post",
            "Hello 世界! 🌍",
        ));
        let records = CarRecords::from_bytes(car_data).unwrap();
        let collected: Result<Vec<_>, _> = records.collect();
        let results = collected.unwrap();
        assert_eq!(results.len(), 1);
        let (_type, cbor_data, _cid) = &results[0];
        let decoded = decode_cbor(cbor_data).unwrap();
        if let CborValue::Map(map) = decoded {
            let text = get_text_field(&map, "text");
            assert_eq!(text, Some("Hello 世界! 🌍"));
        } else {
            panic!("Expected CBOR map");
        }
    }

    #[test]
    fn test_sync_car_reader_empty_car() {
        let car_data = create_car_header();
        let car_reader = SyncCarReader::from_bytes(&car_data).unwrap();
        let entries: Result<Vec<_>, _> = car_reader.collect();
        let results = entries.unwrap();
        assert_eq!(results.len(), 0);
    }

    #[test]
    fn test_sync_car_reader_multiple_entries() {
        let mut car_data = create_car_header();
        car_data.extend_from_slice(&create_at_protocol_entry("app.bsky.feed.post", "First"));
        car_data.extend_from_slice(&create_at_protocol_entry("app.bsky.feed.post", "Second"));
        car_data.extend_from_slice(&create_at_protocol_entry("app.bsky.feed.post", "Third"));
        let car_reader = SyncCarReader::from_bytes(&car_data).unwrap();
        let entries: Result<Vec<_>, _> = car_reader.collect();
        let results = entries.unwrap();
        assert_eq!(results.len(), 3);
        for entry in &results {
            assert_eq!(entry.cid.version, 1);
            assert_eq!(entry.cid.codec, 0x71);
            assert_eq!(entry.cid.digest.len(), 32);
            assert!(!entry.bytes.is_empty());
        }
    }

    #[test]
    fn test_read_cid_raw_codec() {
        let mut cid_data = vec![1, 0x55, 0x12, 32];
        cid_data.extend(vec![0xAB; 32]);
        let mut reader = SyncByteReader::new(&cid_data);
        let cid = read_cid(&mut reader).unwrap();
        assert_eq!(cid.version, 1);
        assert_eq!(cid.codec, 0x55);
        assert_eq!(cid.digest_type, 0x12);
        assert_eq!(cid.digest.len(), 32);
        assert_eq!(cid.digest[0], 0xAB);
    }

    #[test]
    fn test_read_cid_zero_digest_size() {
        let cid_data = vec![1, 0x71, 0x12, 0];
        let mut reader = SyncByteReader::new(&cid_data);
        let cid = read_cid(&mut reader).unwrap();
        assert_eq!(cid.version, 1);
        assert_eq!(cid.digest.len(), 0);
    }

    #[test]
    fn test_car_records_error_recovery() {
        let mut car_data = create_car_header();
        car_data.extend_from_slice(&create_at_protocol_entry(
            "app.bsky.feed.post",
            "Valid entry",
        ));
        // Inject malformed entry (oversized varint)
        car_data.push(0x80);
        car_data.push(0x80);
        car_data.push(0x01);
        let records = CarRecords::from_bytes(car_data).unwrap();
        let mut count = 0;
        for result in records {
            match result {
                Ok(_) => count += 1,
                Err(_) => {
                    assert!(count > 0);
                    return;
                }
            }
        }
        assert_eq!(count, 1);
    }

    #[test]
    fn test_format_cid_simple() {
        let cid = Cid {
            version: 1,
            codec: 0x71,
            digest_type: 0x12,
            digest: vec![0x01, 0x02, 0x03, 0x04],
        };
        let formatted = format_cid_simple(&cid);
        assert!(formatted.starts_with("v1-c71-d12-"));
        assert!(formatted.contains("01020304"));
    }
}
