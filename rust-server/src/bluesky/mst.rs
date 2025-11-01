use crate::car::reader::SyncCarReader;
/// MST (Merkle Search Tree) parsing for extracting CID->rkey mappings
///
/// This module provides functionality to extract CID to rkey mappings from ATProto
/// repository CAR files by parsing the MST structure.
///
/// Based on the atcute implementation for efficient MST traversal.
use crate::car::CarError;
use std::collections::HashMap;

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

    // Build CID -> bytes map from CAR entries
    let mut cid_map: HashMap<String, Vec<u8>> = HashMap::new();
    let mut root_cid: Option<String> = None;

    for entry_result in car_reader {
        let entry = entry_result?;
        let cid_str = format_cid(&entry.cid);
        cid_map.insert(cid_str.clone(), entry.bytes);

        // First CID is typically the commit
        if root_cid.is_none() {
            root_cid = Some(cid_str);
        }
    }

    // Parse the commit to get the data MST root
    let commit_cid =
        root_cid.ok_or_else(|| CarError::InvalidHeader("No root CID found".to_string()))?;
    let data_cid = parse_commit(&cid_map, &commit_cid)?;

    // Walk the MST and collect CID -> rkey mappings
    let mut mappings = HashMap::new();
    walk_mst(&cid_map, &data_cid, collection, &mut mappings)?;

    Ok(mappings)
}

/// Parse commit object to extract data MST root CID
fn parse_commit(cid_map: &HashMap<String, Vec<u8>>, commit_cid: &str) -> Result<String, CarError> {
    let bytes = cid_map
        .get(commit_cid)
        .ok_or_else(|| CarError::InvalidHeader(format!("Commit CID not found: {}", commit_cid)))?;

    let value: serde_cbor::Value = serde_cbor::from_slice(bytes)
        .map_err(|e| CarError::InvalidHeader(format!("Failed to decode commit: {}", e)))?;

    if let serde_cbor::Value::Map(map) = value {
        // Extract "data" field which points to MST root
        for (k, v) in map.iter() {
            if let serde_cbor::Value::Text(key) = k {
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
