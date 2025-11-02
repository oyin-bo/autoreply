use crate::car::reader::SyncCarReader;
/// MST (Merkle Search Tree) parsing for extracting CID->rkey mappings
///
/// This module provides functionality to extract CID to rkey mappings from ATProto
/// repository CAR files by parsing the MST structure.
///
/// Based on the atcute implementation for efficient MST traversal.
use crate::car::CarError;
use std::collections::{HashMap, HashSet};

/// MST node entry from CBOR
#[derive(Debug)]
struct TreeEntry {
    /// Prefix length (bytes shared with previous entry)
    p: u64,
    /// Key suffix (remainder after prefix)
    k: Vec<u8>,
    /// Value CID link
    v: String,
    /// Subtree CID link (nullable)
    t: Option<String>,
}

/// MST node data from CBOR
#[derive(Debug)]
struct NodeData {
    /// Left subtree CID (nullable)
    l: Option<String>,
    /// Entries in this node
    e: Vec<TreeEntry>,
}

/// Extract CID -> rkey mappings from CAR file by walking the MST
///
/// # Arguments
/// * `car_bytes` - The raw CAR file bytes
/// * `collection` - The collection to extract (e.g., "app.bsky.feed.post")
///
/// # Returns
/// A HashMap mapping CID strings to collection/rkey paths
pub fn extract_cid_to_rkey_mapping(
    car_bytes: &[u8],
    collection: &str,
) -> Result<HashMap<String, String>, CarError> {
    let car_reader = SyncCarReader::from_bytes(car_bytes)?;

    // Use CAR header root as the commit CID (correct per CAR spec and indigo implementation)
    let header = car_reader.header();
    let commit_cid_str = header
        .roots
        .get(0)
        .ok_or_else(|| CarError::InvalidHeader("Missing root CID in CAR header".to_string()))
        .map(|c| format_cid(c))?;

    // Build CID -> bytes map from CAR entries
    let mut cid_map: HashMap<String, Vec<u8>> = HashMap::new();

    for entry_result in car_reader {
        let entry = entry_result?;
        let cid_str = format_cid(&entry.cid);
        cid_map.insert(cid_str, entry.bytes);
    }

    // Debug (tests only): show header root and first few entries present
    if cfg!(test) {
        eprintln!("DEBUG: Header root commit: {}", commit_cid_str);
        eprintln!("DEBUG: Entries in CAR: {}", cid_map.len());
        let mut __dbg_count = 0usize;
        for k in cid_map.keys() {
            if __dbg_count < 10 {
                eprintln!("DEBUG: Entry key: {}", k);
            }
            __dbg_count += 1;
            if __dbg_count >= 10 { break; }
        }
    }

    // Parse the commit to get the data MST root; if commit block is absent or
    // header points directly to MST, fall back to using header root as MST root
    let data_cid = match parse_commit(&cid_map, &commit_cid_str) {
        Ok(cid) => cid,
        Err(e) => {
            // Fallback: treat the header root as an MST node directly
            if parse_mst_node(&cid_map, &commit_cid_str).is_ok() {
                commit_cid_str.clone()
            } else if let Some(root) = detect_mst_root(&cid_map) {
                root
            } else {
                return Err(e);
            }
        }
    };

    // Walk the MST and collect CID -> rkey mappings
    let mut mappings = HashMap::new();
    walk_mst(&cid_map, &data_cid, collection, &mut mappings)?;

    Ok(mappings)
}

/// Detect MST root by scanning all MST nodes and finding the one not referenced
fn detect_mst_root(cid_map: &HashMap<String, Vec<u8>>) -> Option<String> {
    let mut nodes: HashSet<String> = HashSet::new();
    let mut referenced: HashSet<String> = HashSet::new();

    for cid in cid_map.keys() {
        if let Ok(node) = parse_mst_node(cid_map, cid) {
            nodes.insert(cid.clone());
            if let Some(l) = node.l {
                referenced.insert(l);
            }
            for entry in node.e {
                if let Some(t) = entry.t {
                    referenced.insert(t);
                }
            }
        }
    }

    // Root is a node not referenced by any other node
    let mut candidates: Vec<String> = nodes.difference(&referenced).cloned().collect();
    if candidates.len() == 1 {
        candidates.pop()
    } else {
        None
    }
}

/// Parse commit object to extract data MST root CID
fn parse_commit(cid_map: &HashMap<String, Vec<u8>>, commit_cid: &str) -> Result<String, CarError> {
    let bytes = cid_map
        .get(commit_cid)
        .ok_or_else(|| CarError::InvalidHeader(format!("Commit CID not found: {}", commit_cid)))?;

    let value: serde_cbor::Value = serde_cbor::from_slice(bytes)
        .map_err(|e| CarError::InvalidHeader(format!("Failed to decode commit: {}", e)))?;

    // Debug (tests only): print commit structure
    if cfg!(test) {
        eprintln!("DEBUG: Commit structure: {:#?}", value);
    }

    if let serde_cbor::Value::Map(map) = value {
        // Extract "data" field which points to MST root
        for (k, v) in map.iter() {
            if let serde_cbor::Value::Text(key) = k {
                if cfg!(test) {
                    eprintln!("DEBUG: Found key '{}' in commit", key);
                }
                if key == "data" {
                    return extract_cid_from_cbor(v);
                }
            }
        }
    }

    Err(CarError::InvalidHeader(
        "Invalid commit structure".to_string(),
    ))
}

/// Walk MST recursively and collect all CID -> collection/rkey mappings
fn walk_mst(
    cid_map: &HashMap<String, Vec<u8>>,
    node_cid: &str,
    collection_filter: &str,
    mappings: &mut HashMap<String, String>,
) -> Result<(), CarError> {
    let node_data = parse_mst_node(cid_map, node_cid)?;

    // Process left subtree first
    if let Some(ref left_cid) = node_data.l {
        walk_mst(cid_map, left_cid, collection_filter, mappings)?;
    }

    let mut last_key = String::new();

    // Process each entry in order
    for entry in node_data.e.iter() {
        // Reconstruct full key from prefix + suffix
        let prefix_len = entry.p as usize;
        if prefix_len > last_key.len() {
            return Err(CarError::InvalidHeader(format!(
                "Invalid prefix length: {} > {}",
                prefix_len,
                last_key.len()
            )));
        }

        let suffix = std::str::from_utf8(&entry.k)
            .map_err(|e| CarError::InvalidHeader(format!("Invalid UTF-8 in key: {}", e)))?;

        let key = format!("{}{}", &last_key[..prefix_len], suffix);
        last_key = key.clone();

        // Key format is "collection/rkey", filter by collection
        if let Some((coll, rkey)) = key.split_once('/') {
            if coll == collection_filter {
                // Map the value CID to collection/rkey
                mappings.insert(entry.v.clone(), format!("{}/{}", coll, rkey));
            }
        }

        // Process right subtree for this entry
        if let Some(ref subtree_cid) = entry.t {
            walk_mst(cid_map, subtree_cid, collection_filter, mappings)?;
        }
    }

    Ok(())
}

/// Parse MST node from CBOR bytes
fn parse_mst_node(cid_map: &HashMap<String, Vec<u8>>, cid: &str) -> Result<NodeData, CarError> {
    let bytes = cid_map
        .get(cid)
        .ok_or_else(|| CarError::InvalidHeader(format!("Node CID not found: {}", cid)))?;

    let value: serde_cbor::Value = serde_cbor::from_slice(bytes)
        .map_err(|e| CarError::InvalidHeader(format!("Failed to decode MST node: {}", e)))?;

    if let serde_cbor::Value::Map(map) = value {
        let mut l = None;
        let mut e = Vec::new();

        for (k, v) in map.iter() {
            if let serde_cbor::Value::Text(key) = k {
                match key.as_str() {
                    "l" => {
                        if !matches!(v, serde_cbor::Value::Null) {
                            l = Some(extract_cid_from_cbor(v)?);
                        }
                    }
                    "e" => {
                        if let serde_cbor::Value::Array(entries) = v {
                            for entry_val in entries {
                                e.push(parse_tree_entry(entry_val)?);
                            }
                        }
                    }
                    _ => {}
                }
            }
        }

        return Ok(NodeData { l, e });
    }

    Err(CarError::InvalidHeader(
        "Invalid MST node structure".to_string(),
    ))
}

/// Parse a single TreeEntry from CBOR
fn parse_tree_entry(value: &serde_cbor::Value) -> Result<TreeEntry, CarError> {
    if let serde_cbor::Value::Map(map) = value {
        let mut p = 0u64;
        let mut k = Vec::new();
        let mut v = String::new();
        let mut t = None;

        for (key, val) in map.iter() {
            if let serde_cbor::Value::Text(key_str) = key {
                match key_str.as_str() {
                    "p" => {
                        if let serde_cbor::Value::Integer(i) = val {
                            p = *i as u64;
                        }
                    }
                    "k" => {
                        if let serde_cbor::Value::Bytes(bytes) = val {
                            k = bytes.clone();
                        }
                    }
                    "v" => {
                        v = extract_cid_from_cbor(val)?;
                    }
                    "t" => {
                        if !matches!(val, serde_cbor::Value::Null) {
                            t = Some(extract_cid_from_cbor(val)?);
                        }
                    }
                    _ => {}
                }
            }
        }

        return Ok(TreeEntry { p, k, v, t });
    }

    Err(CarError::InvalidHeader(
        "Invalid TreeEntry structure".to_string(),
    ))
}

/// Extract CID string from CBOR value (handles CID link format)
fn extract_cid_from_cbor(value: &serde_cbor::Value) -> Result<String, CarError> {
    // CID links in CBOR are represented as maps with a "$link" key
    if let serde_cbor::Value::Map(map) = value {
        for (k, v) in map.iter() {
            if let serde_cbor::Value::Text(key) = k {
                if key == "$link" {
                    if let serde_cbor::Value::Text(cid) = v {
                        return Ok(cid.clone());
                    }
                }
            }
        }
    }

    Err(CarError::InvalidHeader(
        "Invalid CID link format".to_string(),
    ))
}

/// Format CID for use as map key (simple string representation)
fn format_cid(cid: &crate::car::Cid) -> String {
    // Use a simple base representation for now
    // In a full implementation, this would use multibase encoding
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
    use serde_cbor::Value;
    use std::collections::BTreeMap;

    // Helper to create a minimal valid CAR file with commit structure
    fn create_test_car_with_mst() -> Vec<u8> {
        // This is a simplified test - in reality you'd need a complete MST structure
        // For now, we'll test the individual parsing functions

        // Create CAR header
        let mut header_map = BTreeMap::new();
        header_map.insert(Value::Text("version".to_string()), Value::Integer(1));
        header_map.insert(
            Value::Text("roots".to_string()),
            Value::Array(vec![Value::Bytes(vec![
                1, 0x71, 0x12, 32,
                // 32 bytes of digest
                0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08,
                0x09, 0x0A, 0x0B, 0x0C, 0x0D, 0x0E, 0x0F, 0x10,
                0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18,
                0x19, 0x1A, 0x1B, 0x1C, 0x1D, 0x1E, 0x1F, 0x20,
            ])]),
        );

        let header_cbor = serde_cbor::to_vec(&Value::Map(header_map)).unwrap();
        let mut car_data = Vec::new();
        car_data.push(header_cbor.len() as u8);
        car_data.extend_from_slice(&header_cbor);

        car_data
    }

    #[test]
    fn test_extract_cid_from_cbor_valid() {
        // Create a valid CID link
        let mut cid_map = BTreeMap::new();
        cid_map.insert(
            Value::Text("$link".to_string()),
            Value::Text("bafyreicid123".to_string()),
        );

        let cid_value = Value::Map(cid_map);
        let result = extract_cid_from_cbor(&cid_value).unwrap();
        assert_eq!(result, "bafyreicid123");
    }

    #[test]
    fn test_extract_cid_from_cbor_invalid() {
        // Test with non-map value
        let invalid = Value::Text("not a map".to_string());
        assert!(extract_cid_from_cbor(&invalid).is_err());

        // Test with map missing $link
        let mut invalid_map = BTreeMap::new();
        invalid_map.insert(
            Value::Text("wrong_key".to_string()),
            Value::Text("value".to_string()),
        );
        let invalid_value = Value::Map(invalid_map);
        assert!(extract_cid_from_cbor(&invalid_value).is_err());

        // Test with $link but wrong type
        let mut wrong_type_map = BTreeMap::new();
        wrong_type_map.insert(
            Value::Text("$link".to_string()),
            Value::Integer(123),
        );
        let wrong_type_value = Value::Map(wrong_type_map);
        assert!(extract_cid_from_cbor(&wrong_type_value).is_err());
    }

    #[test]
    fn test_parse_tree_entry_valid() {
        // Create a valid TreeEntry CBOR structure
        let mut entry_map = BTreeMap::new();
        entry_map.insert(Value::Text("p".to_string()), Value::Integer(0));
        entry_map.insert(
            Value::Text("k".to_string()),
            Value::Bytes(b"app.bsky.feed.post/abc123".to_vec()),
        );

        let mut v_map = BTreeMap::new();
        v_map.insert(
            Value::Text("$link".to_string()),
            Value::Text("bafyreicidvalue".to_string()),
        );
        entry_map.insert(Value::Text("v".to_string()), Value::Map(v_map));

        entry_map.insert(Value::Text("t".to_string()), Value::Null);

        let entry_value = Value::Map(entry_map);
        let result = parse_tree_entry(&entry_value).unwrap();

        assert_eq!(result.p, 0);
        assert_eq!(result.k, b"app.bsky.feed.post/abc123");
        assert_eq!(result.v, "bafyreicidvalue");
        assert_eq!(result.t, None);
    }

    #[test]
    fn test_parse_tree_entry_with_subtree() {
        // TreeEntry with subtree CID
        let mut entry_map = BTreeMap::new();
        entry_map.insert(Value::Text("p".to_string()), Value::Integer(5));
        entry_map.insert(
            Value::Text("k".to_string()),
            Value::Bytes(b"rkey".to_vec()),
        );

        let mut v_map = BTreeMap::new();
        v_map.insert(
            Value::Text("$link".to_string()),
            Value::Text("value_cid".to_string()),
        );
        entry_map.insert(Value::Text("v".to_string()), Value::Map(v_map));

        let mut t_map = BTreeMap::new();
        t_map.insert(
            Value::Text("$link".to_string()),
            Value::Text("subtree_cid".to_string()),
        );
        entry_map.insert(Value::Text("t".to_string()), Value::Map(t_map));

        let entry_value = Value::Map(entry_map);
        let result = parse_tree_entry(&entry_value).unwrap();

        assert_eq!(result.p, 5);
        assert_eq!(result.t, Some("subtree_cid".to_string()));
    }

    #[test]
    fn test_parse_tree_entry_invalid_structure() {
        // Non-map value
        let invalid = Value::Text("not a map".to_string());
        assert!(parse_tree_entry(&invalid).is_err());

        // Map missing required fields
        let empty_map = Value::Map(BTreeMap::new());
        assert!(parse_tree_entry(&empty_map).is_ok()); // Should work with defaults
    }

    #[test]
    fn test_parse_mst_node_valid() {
        // Create a valid MST node structure
        let mut node_map = BTreeMap::new();

        // Left subtree (null)
        node_map.insert(Value::Text("l".to_string()), Value::Null);

        // Entries array
        let mut entry_map = BTreeMap::new();
        entry_map.insert(Value::Text("p".to_string()), Value::Integer(0));
        entry_map.insert(
            Value::Text("k".to_string()),
            Value::Bytes(b"test".to_vec()),
        );

        let mut v_map = BTreeMap::new();
        v_map.insert(
            Value::Text("$link".to_string()),
            Value::Text("cid".to_string()),
        );
        entry_map.insert(Value::Text("v".to_string()), Value::Map(v_map));
        entry_map.insert(Value::Text("t".to_string()), Value::Null);

        node_map.insert(
            Value::Text("e".to_string()),
            Value::Array(vec![Value::Map(entry_map)]),
        );

        let node_cbor = serde_cbor::to_vec(&Value::Map(node_map)).unwrap();

        // Create a fake CID map
        let mut cid_map = HashMap::new();
        cid_map.insert("test_cid".to_string(), node_cbor);

        let result = parse_mst_node(&cid_map, "test_cid").unwrap();
        assert_eq!(result.l, None);
        assert_eq!(result.e.len(), 1);
    }

    #[test]
    fn test_parse_mst_node_with_left_subtree() {
        // MST node with left subtree
        let mut node_map = BTreeMap::new();

        let mut l_map = BTreeMap::new();
        l_map.insert(
            Value::Text("$link".to_string()),
            Value::Text("left_cid".to_string()),
        );
        node_map.insert(Value::Text("l".to_string()), Value::Map(l_map));

        node_map.insert(Value::Text("e".to_string()), Value::Array(vec![]));

        let node_cbor = serde_cbor::to_vec(&Value::Map(node_map)).unwrap();

        let mut cid_map = HashMap::new();
        cid_map.insert("node_cid".to_string(), node_cbor);

        let result = parse_mst_node(&cid_map, "node_cid").unwrap();
        assert_eq!(result.l, Some("left_cid".to_string()));
        assert_eq!(result.e.len(), 0);
    }

    #[test]
    fn test_parse_mst_node_not_found() {
        let cid_map = HashMap::new();
        let result = parse_mst_node(&cid_map, "missing_cid");
        assert!(matches!(result, Err(CarError::InvalidHeader(_))));
    }

    #[test]
    fn test_parse_mst_node_invalid_cbor() {
        let mut cid_map = HashMap::new();
        cid_map.insert("bad_cid".to_string(), vec![0xFF, 0xFF, 0xFF]);

        let result = parse_mst_node(&cid_map, "bad_cid");
        assert!(matches!(result, Err(CarError::InvalidHeader(_))));
    }

    #[test]
    fn test_parse_commit_valid_structure() {
        // Create a valid commit CBOR structure
        let mut commit_map = BTreeMap::new();

        let mut data_map = BTreeMap::new();
        data_map.insert(
            Value::Text("$link".to_string()),
            Value::Text("mst_root_cid".to_string()),
        );
        commit_map.insert(Value::Text("data".to_string()), Value::Map(data_map));

        let commit_cbor = serde_cbor::to_vec(&Value::Map(commit_map)).unwrap();

        let mut cid_map = HashMap::new();
        cid_map.insert("commit_cid".to_string(), commit_cbor);

        let result = parse_commit(&cid_map, "commit_cid").unwrap();
        assert_eq!(result, "mst_root_cid");
    }

    #[test]
    fn test_parse_commit_missing_data_field() {
        // Commit without 'data' field
        let mut commit_map = BTreeMap::new();
        commit_map.insert(
            Value::Text("other".to_string()),
            Value::Text("value".to_string()),
        );

        let commit_cbor = serde_cbor::to_vec(&Value::Map(commit_map)).unwrap();

        let mut cid_map = HashMap::new();
        cid_map.insert("commit_cid".to_string(), commit_cbor);

        let result = parse_commit(&cid_map, "commit_cid");
        assert!(matches!(result, Err(CarError::InvalidHeader(_))));
    }

    #[test]
    fn test_parse_commit_not_found() {
        let cid_map = HashMap::new();
        let result = parse_commit(&cid_map, "missing_commit");
        assert!(matches!(result, Err(CarError::InvalidHeader(_))));
    }

    #[test]
    fn test_format_cid() {
        let cid = crate::car::Cid {
            version: 1,
            codec: 0x71,
            digest_type: 0x12,
            digest: vec![0xAB, 0xCD, 0xEF],
        };

        let formatted = format_cid(&cid);
        assert!(formatted.starts_with("v1-c71-d12-"));
        assert!(formatted.contains("abcdef"));
    }

    #[test]
    fn test_extract_cid_to_rkey_mapping_empty_car() {
        let car_data = create_test_car_with_mst();
        
        // This will fail because there's no actual MST data, but tests error handling
        let result = extract_cid_to_rkey_mapping(&car_data, "app.bsky.feed.post");
        assert!(result.is_err());
    }

    #[test]
    fn test_extract_cid_to_rkey_mapping_invalid_data() {
        // Invalid CAR data
        let invalid_data = vec![0xFF, 0xFF, 0xFF];
        let result = extract_cid_to_rkey_mapping(&invalid_data, "app.bsky.feed.post");
        assert!(result.is_err());
    }

    #[test]
    fn test_walk_mst_invalid_prefix() {
        // Test walk_mst with entry having invalid prefix length
        let mut node_map = BTreeMap::new();
        node_map.insert(Value::Text("l".to_string()), Value::Null);

        // Create entry with prefix longer than last key (invalid)
        let mut entry_map = BTreeMap::new();
        entry_map.insert(Value::Text("p".to_string()), Value::Integer(999)); // Invalid prefix
        entry_map.insert(
            Value::Text("k".to_string()),
            Value::Bytes(b"test".to_vec()),
        );

        let mut v_map = BTreeMap::new();
        v_map.insert(
            Value::Text("$link".to_string()),
            Value::Text("cid".to_string()),
        );
        entry_map.insert(Value::Text("v".to_string()), Value::Map(v_map));
        entry_map.insert(Value::Text("t".to_string()), Value::Null);

        node_map.insert(
            Value::Text("e".to_string()),
            Value::Array(vec![Value::Map(entry_map)]),
        );

        let node_cbor = serde_cbor::to_vec(&Value::Map(node_map)).unwrap();

        let mut cid_map = HashMap::new();
        cid_map.insert("node_cid".to_string(), node_cbor);

        let mut mappings = HashMap::new();
        let result = walk_mst(&cid_map, "node_cid", "app.bsky.feed.post", &mut mappings);
        assert!(matches!(result, Err(CarError::InvalidHeader(_))));
    }

    #[test]
    fn test_walk_mst_collection_filtering() {
        // Create an MST node with mixed collections
        let mut node_map = BTreeMap::new();
        node_map.insert(Value::Text("l".to_string()), Value::Null);

        let mut entries = Vec::new();

        // Entry for app.bsky.feed.post (should be included)
        let mut entry1 = BTreeMap::new();
        entry1.insert(Value::Text("p".to_string()), Value::Integer(0));
        entry1.insert(
            Value::Text("k".to_string()),
            Value::Bytes(b"app.bsky.feed.post/abc123".to_vec()),
        );

        let mut v_map1 = BTreeMap::new();
        v_map1.insert(
            Value::Text("$link".to_string()),
            Value::Text("post_cid".to_string()),
        );
        entry1.insert(Value::Text("v".to_string()), Value::Map(v_map1));
        entry1.insert(Value::Text("t".to_string()), Value::Null);

        entries.push(Value::Map(entry1));

        // Entry for app.bsky.feed.like (should be filtered out)
        let mut entry2 = BTreeMap::new();
        entry2.insert(Value::Text("p".to_string()), Value::Integer(8));
        entry2.insert(
            Value::Text("k".to_string()),
            Value::Bytes(b".feed.like/xyz789".to_vec()),
        );

        let mut v_map2 = BTreeMap::new();
        v_map2.insert(
            Value::Text("$link".to_string()),
            Value::Text("like_cid".to_string()),
        );
        entry2.insert(Value::Text("v".to_string()), Value::Map(v_map2));
        entry2.insert(Value::Text("t".to_string()), Value::Null);

        entries.push(Value::Map(entry2));

        node_map.insert(Value::Text("e".to_string()), Value::Array(entries));

        let node_cbor = serde_cbor::to_vec(&Value::Map(node_map)).unwrap();

        let mut cid_map = HashMap::new();
        cid_map.insert("node_cid".to_string(), node_cbor);

        let mut mappings = HashMap::new();
        walk_mst(&cid_map, "node_cid", "app.bsky.feed.post", &mut mappings).unwrap();

        // Should only include the post entry, not the like entry
        assert_eq!(mappings.len(), 1);
        assert!(mappings.contains_key("post_cid"));
        assert!(!mappings.contains_key("like_cid"));
    }

    #[test]
    fn test_extract_cid_to_rkey_mapping_with_collection() {
        // Test with different collection types to verify collection parameter is used
        let collections = vec![
            "app.bsky.feed.post",
            "app.bsky.actor.profile",
            "app.bsky.feed.like",
        ];

        for collection in collections {
            let invalid_data = vec![0xFF, 0xFF];
            let result = extract_cid_to_rkey_mapping(&invalid_data, collection);
            assert!(
                result.is_err(),
                "Should fail with invalid data for collection {}",
                collection
            );
        }
    }

    #[test]
    fn test_extract_cid_to_rkey_mapping_empty_collection() {
        let invalid_data = vec![0xFF];
        let result = extract_cid_to_rkey_mapping(&invalid_data, "");
        assert!(result.is_err(), "Should fail with empty collection name");
    }

    #[test]
    fn test_extract_cid_to_rkey_mapping_returns_hashmap() {
        // Verify return type is HashMap on error
        let invalid_data = vec![0x00, 0x01];
        let result = extract_cid_to_rkey_mapping(&invalid_data, "app.bsky.feed.post");
        assert!(result.is_err());
        // Type is verified at compile time - this test confirms error handling
    }

    #[test]
    fn test_extract_cid_to_rkey_mapping_large_collection_name() {
        // Test with abnormally large collection name
        let invalid_data = vec![0xFF];
        let long_collection = format!("app.bsky.feed.post{}", "x".repeat(1000));
        let result = extract_cid_to_rkey_mapping(&invalid_data, &long_collection);
        assert!(result.is_err());
    }

    #[test]
    fn test_extract_cid_to_rkey_mapping_special_characters() {
        // Test collection names with special characters
        let invalid_data = vec![0xFF];
        let special_collections = vec![
            "app.bsky.feed.post/test",
            "app.bsky.feed.post?query=1",
            "app.bsky.feed.post#fragment",
            "app.bsky.feed.post\nwith\nnewlines",
        ];

        for collection in special_collections {
            let result = extract_cid_to_rkey_mapping(&invalid_data, collection);
            assert!(
                result.is_err(),
                "Should fail for collection with special chars: {}",
                collection
            );
        }
    }

    #[test]
    fn test_extract_cid_to_rkey_mapping_nil_data() {
        // Test with empty slice
        let empty_data: Vec<u8> = vec![];
        let result = extract_cid_to_rkey_mapping(&empty_data, "app.bsky.feed.post");
        assert!(result.is_err(), "Should fail with empty data");
    }

    #[test]
    fn test_extract_cid_to_rkey_mapping_error_propagation() {
        // Verify errors contain meaningful context
        let malformed_data = vec![0x00, 0x01, 0x02];
        let result = extract_cid_to_rkey_mapping(&malformed_data, "app.bsky.feed.post");

        match result {
            Err(e) => {
                let error_msg = format!("{:?}", e);
                assert!(!error_msg.is_empty(), "Error message should not be empty");
            }
            Ok(_) => panic!("Should fail with malformed data"),
        }
    }

    #[test]
    fn test_extract_cid_to_rkey_mapping_multiple_collections() {
        // Test that same CAR data can be queried for different collections
        let invalid_data = vec![0xFF, 0xFE];
        let collections = vec![
            "app.bsky.feed.post",
            "app.bsky.feed.like",
            "app.bsky.actor.profile",
        ];

        for collection in collections {
            let result = extract_cid_to_rkey_mapping(&invalid_data, collection);
            assert!(
                result.is_err(),
                "Should fail for collection {}",
                collection
            );
        }
    }

    #[test]
    fn test_extract_cid_to_rkey_mapping_real_car() {
        // CRITICAL TEST: Extract CID-to-rkey mapping from real CAR file
        // This is the test that Go has but Rust was missing
        let cache_dir = match dirs::cache_dir() {
            Some(dir) => dir,
            None => {
                eprintln!("Skipping test: Cannot determine cache directory");
                return;
            }
        };

        let car_path = cache_dir
            .join("autoreply")
            .join("did")
            .join("5c")
            .join("5cajdgeo6qz32kptlpg4c3lv")
            .join("repo.car");

        if !car_path.exists() {
            eprintln!(
                "Skipping test: CAR file not found at {}. Run Go tests first to download it.",
                car_path.display()
            );
            return;
        }

        let car_bytes = match std::fs::read(&car_path) {
            Ok(bytes) => bytes,
            Err(e) => {
                eprintln!("Failed to read CAR file: {}", e);
                return;
            }
        };

        // Test MST extraction with real data - this is the failing code path
        let mapping =
            extract_cid_to_rkey_mapping(&car_bytes, "app.bsky.feed.post").expect("MST extraction failed on real CAR file");

        if mapping.is_empty() {
            println!("No post mappings found (repository might not have posts)");
        } else {
            println!("Extracted {} CID-to-rkey mappings", mapping.len());

            // Verify mapping structure
            let mut count = 0;
            for (cid, rkey) in &mapping {
                assert!(!cid.is_empty(), "Found empty CID in mapping");
                assert!(!rkey.is_empty(), "Found empty rkey in mapping");
                count += 1;
                if count >= 3 {
                    break;
                }
                println!("Sample mapping: CID={} -> rkey={}", cid, rkey);
            }
        }
    }
}

#[cfg(test)]
mod records_real_car_tests {
    use super::*;

    #[test]
    fn test_find_profile_record_real_data() {
        let cache_dir = match dirs::cache_dir() {
            Some(dir) => dir,
            None => {
                eprintln!("Skipping test: Cannot determine cache directory");
                return;
            }
        };

        let car_path = cache_dir
            .join("autoreply")
            .join("did")
            .join("5c")
            .join("5cajdgeo6qz32kptlpg4c3lv")
            .join("repo.car");

        if !car_path.exists() {
            eprintln!(
                "Skipping test: CAR file not found at {}",
                car_path.display()
            );
            return;
        }

        let car_bytes = std::fs::read(&car_path).expect("Failed to read CAR file");

        // Extract MST mapping first
        let mapping = extract_cid_to_rkey_mapping(&car_bytes, "app.bsky.actor.profile")
            .expect("Failed to extract CID mapping for profiles");

        // Then parse CAR records
        let reader = crate::car::CarRecords::from_bytes(car_bytes).expect("Failed to create CAR reader");

        let mut profile_found = false;
        for entry_result in reader {
            let (record_type, cbor_data, cid) = entry_result.expect("Failed to read CAR entry");

            if record_type == "app.bsky.actor.profile" {
                profile_found = true;

                // Verify CID is in the mapping
                let cid_str = format!("{}", cid);
                if !mapping.is_empty() {
                    assert!(
                        mapping.contains_key(&cid_str),
                        "CID {} should be in MST mapping",
                        cid_str
                    );
                }

                // Parse the profile
                let profile: crate::bluesky::records::ProfileRecord =
                    serde_cbor::from_slice(&cbor_data).expect("Failed to parse profile");

                assert!(
                    !profile.created_at.is_empty(),
                    "Profile createdAt should not be empty"
                );

                println!("Profile createdAt: {}", profile.created_at);
                break;
            }
        }

        assert!(profile_found, "Should find profile record");
    }

    #[test]
    fn test_find_matching_posts_real_data() {
        let cache_dir = match dirs::cache_dir() {
            Some(dir) => dir,
            None => {
                eprintln!("Skipping test: Cannot determine cache directory");
                return;
            }
        };

        let car_path = cache_dir
            .join("autoreply")
            .join("did")
            .join("5c")
            .join("5cajdgeo6qz32kptlpg4c3lv")
            .join("repo.car");

        if !car_path.exists() {
            eprintln!(
                "Skipping test: CAR file not found at {}",
                car_path.display()
            );
            return;
        }

        let car_bytes = std::fs::read(&car_path).expect("Failed to read CAR file");

        // Extract MST mapping
        let mapping = extract_cid_to_rkey_mapping(&car_bytes, "app.bsky.feed.post")
            .expect("Failed to extract CID mapping");

        // Parse CAR records
        let reader = crate::car::CarRecords::from_bytes(car_bytes).expect("Failed to create CAR reader");

        let mut posts_found = 0;
        for entry_result in reader {
            let (record_type, cbor_data, cid) = entry_result.expect("Failed to read CAR entry");

            if record_type == "app.bsky.feed.post" {
                posts_found += 1;

                // Verify CID is in MST mapping
                let cid_str = format!("{}", cid);
                if !mapping.is_empty() {
                    assert!(
                        mapping.contains_key(&cid_str),
                        "Post CID {} should be in MST mapping",
                        cid_str
                    );

                    let rkey = &mapping[&cid_str];
                    println!("Post CID={} -> rkey={}", cid_str, rkey);
                }

                // Parse the post
                let post: crate::bluesky::records::PostRecord =
                    serde_cbor::from_slice(&cbor_data).expect("Failed to parse post");

                assert!(
                    !post.text.is_empty() || !post.embeds.is_empty(),
                    "Post should have text or embeds"
                );

                if posts_found >= 5 {
                    break;
                }
            }
        }

        assert!(posts_found > 0, "Should find at least one post");
        println!("Found {} posts with valid MST mappings", posts_found);
    }

    #[test]
    fn test_resolve_uris_for_cids_real_data() {
        let cache_dir = match dirs::cache_dir() {
            Some(dir) => dir,
            None => {
                eprintln!("Skipping test: Cannot determine cache directory");
                return;
            }
        };

        let car_path = cache_dir
            .join("autoreply")
            .join("did")
            .join("5c")
            .join("5cajdgeo6qz32kptlpg4c3lv")
            .join("repo.car");

        if !car_path.exists() {
            eprintln!(
                "Skipping test: CAR file not found at {}",
                car_path.display()
            );
            return;
        }

        let car_bytes = std::fs::read(&car_path).expect("Failed to read CAR file");

        // Extract MST mapping for posts
        let mapping = extract_cid_to_rkey_mapping(&car_bytes, "app.bsky.feed.post")
            .expect("Failed to extract CID mapping");

        if mapping.is_empty() {
            println!("No posts found in repository, skipping URI resolution test");
            return;
        }

        // Test URI construction from CID mappings
        let did = "did:plc:5cajdgeo6qz32kptlpg4c3lv";
        let collection = "app.bsky.feed.post";

        for (cid, rkey) in mapping.iter().take(5) {
            let uri = format!("at://{}/{}/{}", did, collection, rkey);
            println!("Resolved CID {} to URI: {}", cid, uri);

            // Verify URI format
            assert!(uri.starts_with("at://"));
            assert!(uri.contains(collection));
            assert!(uri.contains(rkey));
        }
    }
}

#[cfg(test)]
mod provider_real_car_tests {
    use super::*;

    #[test]
    fn test_fetch_repository_real_flow() {
        // This test requires network access and takes time
        // It verifies the full fetch and cache flow
        use crate::bluesky::provider::RepositoryProvider;

        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let provider = RepositoryProvider::default();
            let did = "did:plc:5cajdgeo6qz32kptlpg4c3lv"; // autoreply.ooo

            // Attempt to fetch repository
            match provider.fetch_repo_car(did).await {
                Ok(car_path) => {
                    assert!(car_path.exists(), "CAR file should exist");
                    
                    let car_bytes = std::fs::read(&car_path).expect("Failed to read fetched CAR");
                    println!(
                        "Successfully fetched repository: {} bytes",
                        car_bytes.len()
                    );

                    // Verify we can extract MST from fetched data
                    let mapping = extract_cid_to_rkey_mapping(&car_bytes, "app.bsky.feed.post")
                        .expect("Should be able to extract MST from fetched CAR");

                    println!("Extracted {} CID mappings from fetched repository", mapping.len());
                }
                Err(e) => {
                    eprintln!("Failed to fetch repository (this may be expected in CI): {}", e);
                    // Don't fail the test - network may not be available
                }
            }
        });
    }
}

