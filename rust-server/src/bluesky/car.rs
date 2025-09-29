//! CAR file operations and repository fetching
//!
//! Handles downloading and processing CAR files from Bluesky

use crate::bluesky::records::{ProfileRecord, PostRecord, Embed, ExternalEmbed, ImageEmbed, BlobRef, Facet, FacetFeature, FacetIndex};
use crate::cache::{CacheManager, CacheMetadata};
use crate::error::AppError;
use anyhow::Result;
use reqwest::Client;
use std::time::Duration;
use tokio_stream::StreamExt;
use tracing::{debug, info, warn};
use crate::bluesky::did::DidResolver;
use serde_cbor::Value as CborValue;
use std::collections::BTreeMap;
use libipld::cid::Cid;
use libipld::multihash::Multihash;
use serde::Deserialize;
use std::collections::{HashMap, HashSet};

/// Repository fetcher and CAR processor
pub struct CarProcessor {
    client: Client,
    cache: CacheManager,
    did_resolver: DidResolver,
}

/// Read CID at the current index in block and return string, advancing idx
fn read_cid_string(block: &[u8], idx: &mut usize) -> Option<String> {
    let mut i = *idx;
    let _ver = read_uvarint(block, &mut i)?;
    let codec = read_uvarint(block, &mut i)?;
    let mh_code = read_uvarint(block, &mut i)?;
    let dlen = read_uvarint(block, &mut i)? as usize;
    if i + dlen > block.len() { return None; }
    let digest = &block[i..i + dlen];
    i += dlen;
    *idx = i;
    let mh = Multihash::wrap(mh_code as u64, digest).ok()?;
    let cid = Cid::new_v1(codec as u64, mh);
    Some(cid.to_string())
}

/// Helpers to parse nested CBOR
fn cbor_get_map<'a>(map: &'a BTreeMap<CborValue, CborValue>, key: &str) -> Option<&'a BTreeMap<CborValue, CborValue>> {
    map.get(&CborValue::Text(key.to_string())).and_then(|v| match v {
        CborValue::Map(m) => Some(m),
        _ => None,
    })
}

fn cbor_get_array<'a>(map: &'a BTreeMap<CborValue, CborValue>, key: &str) -> Option<&'a Vec<CborValue>> {
    map.get(&CborValue::Text(key.to_string())).and_then(|v| match v {
        CborValue::Array(a) => Some(a),
        _ => None,
    })
}

fn cbor_get_u64(map: &BTreeMap<CborValue, CborValue>, key: &str) -> Option<u64> {
    map.get(&CborValue::Text(key.to_string())).and_then(|v| match v {
        CborValue::Integer(i) => (*i).try_into().ok(),
        _ => None,
    })
}

fn parse_embeds(map: &BTreeMap<CborValue, CborValue>) -> Vec<Embed> {
    let mut embeds = Vec::new();
    if let Some(emb_val) = map.get(&CborValue::Text("embed".to_string())) {
        match emb_val {
            CborValue::Map(emb_map) => {
                if let Some(etype) = cbor_get_text(emb_map, "$type") {
                    match etype.as_str() {
                        "app.bsky.embed.external" => {
                            if let Some(ext) = cbor_get_map(emb_map, "external") {
                                let uri = cbor_get_text(ext, "uri").unwrap_or_default();
                                let title = cbor_get_text(ext, "title").unwrap_or_default();
                                let description = cbor_get_text(ext, "description").unwrap_or_default();
                                embeds.push(Embed::External { external: ExternalEmbed { uri, title, description, thumb: None } });
                            }
                        }
                        "app.bsky.embed.images" => {
                            if let Some(images) = cbor_get_array(emb_map, "images") {
                                let mut imgs = Vec::new();
                                for img_val in images {
                                    if let CborValue::Map(img_map) = img_val {
                                        let alt = cbor_get_text(img_map, "alt");
                                        let image = cbor_get_map(img_map, "image");
                                        let (mime_type, size) = if let Some(im) = image { (cbor_get_text(im, "mimeType").unwrap_or_default(), cbor_get_u64(im, "size").unwrap_or(0)) } else { (String::new(), 0) };
                                        let blob = BlobRef { type_: "blob".to_string(), ref_: String::new(), mime_type, size };
                                        imgs.push(ImageEmbed { alt, image: blob });
                                    }
                                }
                                embeds.push(Embed::Images { images: imgs });
                            }
                        }
                        "app.bsky.embed.recordWithMedia" => {
                            // Parse media part if present
                            if let Some(media) = cbor_get_map(emb_map, "media") {
                                let mut tmp_root = BTreeMap::new();
                                tmp_root.insert(CborValue::Text("embed".to_string()), CborValue::Map(media.clone()));
                                embeds.extend(parse_embeds(&tmp_root));
                            }
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }
    embeds
}

fn parse_facets(map: &BTreeMap<CborValue, CborValue>) -> Vec<Facet> {
    let mut facets_vec = Vec::new();
    if let Some(arr) = cbor_get_array(map, "facets") {
        for item in arr {
            if let CborValue::Map(fm) = item {
                // index
                let index = if let Some(idx_map) = cbor_get_map(fm, "index") {
                    let bs = cbor_get_u64(idx_map, "byteStart").unwrap_or(0) as u32;
                    let be = cbor_get_u64(idx_map, "byteEnd").unwrap_or(0) as u32;
                    FacetIndex { byte_start: bs, byte_end: be }
                } else { FacetIndex { byte_start: 0, byte_end: 0 } };
                // features
                let mut feats = Vec::new();
                if let Some(features) = cbor_get_array(fm, "features") {
                    for feat in features {
                        if let CborValue::Map(ff) = feat {
                            if let Some(ftype) = cbor_get_text(ff, "$type") {
                                if ftype == "app.bsky.richtext.facet#link" {
                                    if let Some(uri) = cbor_get_text(ff, "uri") {
                                        feats.push(FacetFeature::Link { uri });
                                    }
                                }
                            }
                        }
                    }
                }
                facets_vec.push(Facet { index, features: feats });
            }
        }
    }
    facets_vec
}

/// Helper: get string value for a key from CBOR Map
fn cbor_get_text(map: &BTreeMap<CborValue, CborValue>, key: &str) -> Option<String> {
    map.get(&CborValue::Text(key.to_string())).and_then(|v| match v {
        CborValue::Text(s) => Some(s.clone()),
        _ => None,
    })
}

impl CarProcessor {
    /// Create new CAR processor
    pub fn new() -> Result<Self, AppError> {
        let client = crate::http::client_with_timeout(Duration::from_secs(60));

        let cache = CacheManager::new()?;

        let did_resolver = DidResolver::new();

        Ok(Self { client, cache, did_resolver })
    }

    /// Parse CAR v1 and collect app.bsky.feed.post records
    fn find_posts_in_car(&self, car_data: &[u8]) -> Result<Vec<PostRecord>, AppError> {
        let mut idx = 0usize;
        // Header: varint length, followed by that many bytes of header CBOR
        let Some(hlen) = read_uvarint(car_data, &mut idx) else {
            return Err(AppError::RepoParseFailed("Invalid CAR header length".to_string()));
        };
        let hlen = hlen as usize;
        if idx + hlen > car_data.len() { return Err(AppError::RepoParseFailed("Truncated CAR header".to_string())); }
        idx += hlen;

        let mut posts: Vec<PostRecord> = Vec::with_capacity(1024);

        while idx < car_data.len() {
            let start = idx;
            let Some(blen) = read_uvarint(car_data, &mut idx) else { break };
            let blen = blen as usize;
            if idx + blen > car_data.len() { break; }
            let block = &car_data[idx .. idx + blen];
            idx += blen;

            // Split CID and data, capture CID string
            let mut bidx = 0usize;
            let cid_str = read_cid_string(block, &mut bidx).unwrap_or_default();
            if bidx >= block.len() { continue; }
            let data = &block[bidx..];

            if let Ok(val) = serde_cbor::from_slice::<CborValue>(data) {
                if let CborValue::Map(map) = val {
                    if let Some(ctype) = cbor_get_text(&map, "$type") {
                        if ctype == "app.bsky.feed.post" {
                            let text = cbor_get_text(&map, "text").unwrap_or_default();
                            let created_at = cbor_get_text(&map, "createdAt").unwrap_or_default();
                            let embeds = parse_embeds(&map);
                            let facets = parse_facets(&map);
                            posts.push(PostRecord {
                                uri: String::new(),
                                cid: cid_str.clone(),
                                text,
                                created_at,
                                embeds,
                                facets,
                            });
                        }
                    }
                }
            }

            // Safety: avoid infinite loop
            if idx <= start { break; }
            if posts.len() >= 100_000 { break; }
        }

        Ok(posts)
    }

    /// Fetch repository for DID, using cache if valid
    pub async fn fetch_repo(&self, did: &str) -> Result<Vec<u8>, AppError> {
        // Check cache first (TTL: 24 hours for repos as specified)
        if self.cache.is_cache_valid(did, 24) {
            debug!("Using cached CAR file for DID: {}", did);
            return self.cache.read_car(did);
        }

        info!("Fetching CAR file for DID: {}", did);

        // Discover PDS endpoint for this DID.
        // - For did:web: use did:web document serviceEndpoint if available
        // - For did:plc: inspect PLC audit log
        // - Fallback: bsky.social
        let mut base = "https://bsky.social".to_string();
        if did.starts_with("did:web:") {
            // try cached value first
            if let Some(pds) = self.did_resolver.get_pds_for(did).await {
                base = pds;
            } else {
                match self.did_resolver.ensure_did_web_pds(did).await {
                    Ok(Some(pds)) => base = pds,
                    Ok(None) => warn!("No PDS in did:web document for {}, using fallback", did),
                    Err(e) => warn!("Error reading did:web document for {}: {}. Using fallback.", did, e),
                }
            }
        } else {
            match self.did_resolver.discover_pds(did).await {
                Ok(Some(pds)) => {
                    base = pds;
                }
                Ok(None) => {
                    warn!("No PDS discovered for {}, falling back to bsky.social", did);
                }
                Err(e) => {
                    warn!("Error discovering PDS for {}: {}. Falling back to bsky.social", did, e);
                }
            }
        }

        // Compose CAR fetch URL using discovered or fallback base
        let url = format!("{}/xrpc/com.atproto.sync.getRepo?did={}", base.trim_end_matches('/'), did);

        debug!("Fetching repo from URL: {}", url);

        let response = self.client.get(&url).send().await?;
        
        if !response.status().is_success() {
            return Err(AppError::RepoFetchFailed(format!(
                "HTTP {} from repo fetch: {}",
                response.status(),
                response.text().await.unwrap_or_default()
            )));
        }

        // Extract headers for cache validation
        let etag = response
            .headers()
            .get("etag")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string());
            
        let last_modified = response
            .headers()
            .get("last-modified")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string());
            
        let content_length = response
            .headers()
            .get("content-length")
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.parse().ok());

        // Stream download with progress tracking as specified
        let mut car_data = Vec::new();
        let mut stream = response.bytes_stream();
        
        while let Some(chunk) = stream.next().await {
            let chunk = chunk?;
            car_data.extend_from_slice(&chunk);
            
            if car_data.len() % (1024 * 1024) == 0 {
                debug!("Downloaded {} MB", car_data.len() / (1024 * 1024));
            }
        }

        info!("Downloaded CAR file: {} bytes", car_data.len());

        // Validate content length if provided
        if let Some(expected_len) = content_length {
            if car_data.len() as u64 != expected_len {
                return Err(AppError::RepoFetchFailed(format!(
                    "Content length mismatch: expected {}, got {}",
                    expected_len,
                    car_data.len()
                )));
            }
        }

        // Store in cache with metadata
        let metadata = CacheMetadata::new(did.to_string(), 24)
            .with_headers(etag, last_modified, content_length);
            
        self.cache.store_car(did, &car_data, metadata)?;

        Ok(car_data)
    }

    /// Extract profile records from CAR data
    pub async fn extract_profile(&self, car_data: &[u8]) -> Result<Option<ProfileRecord>, AppError> {
        // Scan CAR blocks for app.bsky.actor.profile record
        let profile = self.find_profile_in_car(car_data)?;
        Ok(profile)
    }

    /// Parse CAR v1 and find the latest app.bsky.actor.profile record
    fn find_profile_in_car(&self, car_data: &[u8]) -> Result<Option<ProfileRecord>, AppError> {
        let mut idx = 0usize;
        let Some(hlen) = read_uvarint(car_data, &mut idx) else {
            return Err(AppError::RepoParseFailed("Invalid CAR header length".to_string()));
        };
        let hlen = hlen as usize;
        if idx + hlen > car_data.len() { return Err(AppError::RepoParseFailed("Truncated CAR header".to_string())); }
        idx += hlen;

        let mut found: Option<ProfileRecord> = None;
        // Iterate blocks
        while idx < car_data.len() {
            let start = idx;
            let Some(blen) = read_uvarint(car_data, &mut idx) else { break };
            let blen = blen as usize;
            if idx + blen > car_data.len() { break; }
            let block = &car_data[idx .. idx + blen];
            idx += blen;

            // Inside block: CID (varint-coded fields) + DAG-CBOR data
            let mut bidx = 0usize;
            if skip_cid(block, &mut bidx).is_none() { continue; }
            if bidx >= block.len() { continue; }
            let data = &block[bidx..];

            // Try decode DAG-CBOR map
            if let Ok(val) = serde_cbor::from_slice::<CborValue>(data) {
                if let CborValue::Map(map) = val {
                    if let Some(ctype) = cbor_get_text(&map, "$type") {
                        if ctype == "app.bsky.actor.profile" {
                            // Build ProfileRecord from known fields
                            let display_name = cbor_get_text(&map, "displayName");
                            let description = cbor_get_text(&map, "description");
                            let avatar = cbor_get_text(&map, "avatar");
                            let banner = cbor_get_text(&map, "banner");
                            let created_at = cbor_get_text(&map, "createdAt").unwrap_or_default();
                            found = Some(ProfileRecord { display_name, description, avatar, banner, created_at });
                        }
                    }
                }
            }
            // Safety: avoid infinite loop
            if idx <= start { break; }
        }

        Ok(found)
    }

    /// Extract post records from CAR data (parse-only)
    pub async fn extract_posts(&self, car_data: &[u8]) -> Result<Vec<PostRecord>, AppError> {
        let posts = self.find_posts_in_car(car_data)?;
        Ok(posts)
    }

    /// Resolve URIs for a set of CIDs by calling listRecords and returning a cid->uri map
    pub async fn resolve_uris_for_cids(&self, did: &str, needed: &HashSet<String>) -> Result<HashMap<String, String>, AppError> {
        let mut needed = needed.clone();
        if needed.is_empty() { return Ok(HashMap::new()); }

        // Discover PDS base (reuse same logic as fetch_repo)
        let mut base = "https://bsky.social".to_string();
        if did.starts_with("did:web:") {
            if let Some(pds) = self.did_resolver.get_pds_for(did).await { base = pds; }
            else if let Ok(Some(pds)) = self.did_resolver.ensure_did_web_pds(did).await { base = pds; }
        } else if let Ok(Some(pds)) = self.did_resolver.discover_pds(did).await { base = pds; }

        #[derive(Debug, Deserialize)]
        struct RecordItem { uri: String, cid: String }
        #[derive(Debug, Deserialize)]
        struct ListResp { cursor: Option<String>, records: Vec<RecordItem> }

        let mut cursor: Option<String> = None;
        let mut map: HashMap<String, String> = HashMap::new(); // cid -> uri
        let mut pages = 0usize;

        while !needed.is_empty() && pages < 25 {
            let mut url = format!(
                "{}/xrpc/com.atproto.repo.listRecords?repo={}&collection=app.bsky.feed.post&limit=100",
                base.trim_end_matches('/'), did
            );
            if let Some(c) = &cursor { url.push_str(&format!("&cursor={}", c)); }

            let resp = self.client.get(&url).send().await?;
            if !resp.status().is_success() { break; }
            let parsed: ListResp = resp.json().await.unwrap_or(ListResp { cursor: None, records: Vec::new() });
            for item in parsed.records.into_iter() {
                if needed.contains(&item.cid) {
                    map.insert(item.cid.clone(), item.uri.clone());
                    needed.remove(&item.cid);
                }
            }
            cursor = parsed.cursor;
            if cursor.is_none() { break; }
            pages += 1;
        }

        Ok(map)
    }

}

/// Read unsigned varint from data starting at idx, advancing idx
fn read_uvarint(data: &[u8], idx: &mut usize) -> Option<u64> {
    let mut x: u64 = 0;
    let mut s: u32 = 0;
    let mut i = *idx;
    while i < data.len() {
        let b = data[i];
        if b < 0x80 {
            if s >= 64 { return None; }
            x |= ((b & 0x7F) as u64) << s;
            i += 1;
            *idx = i;
            return Some(x);
        }
        x |= ((b & 0x7F) as u64) << s;
        s += 7;
        i += 1;
        if s > 63 { return None; }
    }
    None
}


/// Skip CID in block bytes by parsing CIDv1 structure; advances idx to start of block data
fn skip_cid(block: &[u8], idx: &mut usize) -> Option<()> {
    let mut i = *idx;
    // version
    let Some(_ver) = read_uvarint(block, &mut i) else { return None };
    // codec
    let Some(_codec) = read_uvarint(block, &mut i) else { return None };
    // multihash code
    let Some(_mh_code) = read_uvarint(block, &mut i) else { return None };
    // digest length
    let Some(dlen) = read_uvarint(block, &mut i) else { return None };
    let dlen = dlen as usize;
    if i + dlen > block.len() { return None; }
    i += dlen;
    *idx = i;
    Some(())
}

impl Default for CarProcessor {
    fn default() -> Self {
        Self::new().expect("Failed to create CarProcessor")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    use serde_cbor::Value as CborValue;

    /// Create a minimal CAR file with header and one or more blocks for testing
    fn create_test_car_data(blocks: Vec<(&str, &[u8])>) -> Vec<u8> {
        let mut car_data = Vec::new();
        
        // CAR header: {"version": 1, "roots": ["bafy..."]} encoded as DAG-CBOR
        let header = b"\x81\xa2gversion\x01erootsx\x81xIbafybeigdyrzt5sfp7udm7hu76uh7y26nf3efuylqabf3oclgtqy55fbzdi";
        
        // Write header length as varint and header bytes
        write_varint(&mut car_data, header.len() as u64);
        car_data.extend_from_slice(header);
        
        // Write blocks
        for (cid_hex, data) in blocks {
            // Build a valid CIDv1 prefix: ver=1, codec=0x70 (dag-cbor), mh_code=0x12 (sha2-256), dlen=32
            let mut cid_bytes = Vec::new();
            write_varint(&mut cid_bytes, 1);
            write_varint(&mut cid_bytes, 0x70);
            write_varint(&mut cid_bytes, 0x12);
            write_varint(&mut cid_bytes, 32);

            // Use provided hex as digest when at least 32 bytes, otherwise zero-fill
            let decoded = hex::decode(cid_hex).unwrap_or_default();
            if decoded.len() >= 32 {
                cid_bytes.extend_from_slice(&decoded[..32]);
            } else {
                let mut digest = [0u8; 32];
                if !decoded.is_empty() {
                    let n = decoded.len().min(32);
                    digest[..n].copy_from_slice(&decoded[..n]);
                }
                cid_bytes.extend_from_slice(&digest);
            }

            // Assemble block: CID bytes + CBOR payload
            let mut block_data = Vec::new();
            block_data.extend_from_slice(&cid_bytes);
            block_data.extend_from_slice(data);

            // Write block length and block bytes
            write_varint(&mut car_data, block_data.len() as u64);
            car_data.extend_from_slice(&block_data);
        }
        
        car_data
    }
    
    fn write_varint(buf: &mut Vec<u8>, mut x: u64) {
        while x >= 0x80 {
            buf.push((x as u8) | 0x80);
            x >>= 7;
        }
        buf.push(x as u8);
    }

    #[test]
    fn test_read_uvarint() {
        let data = [0x08, 0x96, 0x01, 0xFF, 0xFF, 0xFF, 0xFF, 0x0F];
        let mut idx = 0;
        
        assert_eq!(read_uvarint(&data, &mut idx), Some(8));
        assert_eq!(idx, 1);
        
        assert_eq!(read_uvarint(&data, &mut idx), Some(150));
        assert_eq!(idx, 3);
        
        assert_eq!(read_uvarint(&data, &mut idx), Some(0xFFFFFFFF));
        assert_eq!(idx, 8);
        
        // Test overflow - this should be rejected due to too many continuation bytes
        let overflow_data = [0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF];
        let mut idx = 0;
        assert_eq!(read_uvarint(&overflow_data, &mut idx), None);
    }

    #[test]
    fn test_skip_cid() {
        // Create a test CID: version(1) + codec(0x70) + multihash_code(0x12) + digest_len(32) + digest(32 bytes)
        let mut test_data = Vec::new();
        write_varint(&mut test_data, 1); // version
        write_varint(&mut test_data, 0x70); // codec
        write_varint(&mut test_data, 0x12); // multihash code
        write_varint(&mut test_data, 32); // digest length
        test_data.extend_from_slice(&[0u8; 32]); // 32-byte digest
        test_data.extend_from_slice(b"payload_data");
        
        let mut idx = 0;
        assert!(skip_cid(&test_data, &mut idx).is_some());
        assert_eq!(&test_data[idx..], b"payload_data");
    }

    #[test]
    fn test_cbor_get_text() {
        let mut map = BTreeMap::new();
        map.insert(CborValue::Text("key".to_string()), CborValue::Text("value".to_string()));
        map.insert(CborValue::Text("number".to_string()), CborValue::Integer(42));
        
        assert_eq!(cbor_get_text(&map, "key"), Some("value".to_string()));
        assert_eq!(cbor_get_text(&map, "number"), None);
        assert_eq!(cbor_get_text(&map, "missing"), None);
    }

    #[test]
    fn test_cbor_get_u64() {
        let mut map = BTreeMap::new();
        map.insert(CborValue::Text("positive".to_string()), CborValue::Integer(42));
        map.insert(CborValue::Text("zero".to_string()), CborValue::Integer(0));
        map.insert(CborValue::Text("negative".to_string()), CborValue::Integer(-1));
        map.insert(CborValue::Text("text".to_string()), CborValue::Text("not_number".to_string()));
        
        assert_eq!(cbor_get_u64(&map, "positive"), Some(42));
        assert_eq!(cbor_get_u64(&map, "zero"), Some(0));
        assert_eq!(cbor_get_u64(&map, "negative"), None); // negative not convertible to u64
        assert_eq!(cbor_get_u64(&map, "text"), None);
        assert_eq!(cbor_get_u64(&map, "missing"), None);
    }

    #[test]
    fn test_parse_embeds_external() {
        let mut embed_map = BTreeMap::new();
        embed_map.insert(CborValue::Text("$type".to_string()), CborValue::Text("app.bsky.embed.external".to_string()));
        
        let mut external_map = BTreeMap::new();
        external_map.insert(CborValue::Text("uri".to_string()), CborValue::Text("https://example.com".to_string()));
        external_map.insert(CborValue::Text("title".to_string()), CborValue::Text("Test Title".to_string()));
        external_map.insert(CborValue::Text("description".to_string()), CborValue::Text("Test Description".to_string()));
        embed_map.insert(CborValue::Text("external".to_string()), CborValue::Map(external_map));
        
        let mut root_map = BTreeMap::new();
        root_map.insert(CborValue::Text("embed".to_string()), CborValue::Map(embed_map));
        
        let embeds = parse_embeds(&root_map);
        assert_eq!(embeds.len(), 1);
        
        match &embeds[0] {
            Embed::External { external } => {
                assert_eq!(external.uri, "https://example.com");
                assert_eq!(external.title, "Test Title");
                assert_eq!(external.description, "Test Description");
            }
            _ => panic!("Expected External embed"),
        }
    }

    #[test]
    fn test_parse_facets_with_links() {
        let mut facet_map = BTreeMap::new();
        
        // Create index
        let mut index_map = BTreeMap::new();
        index_map.insert(CborValue::Text("byteStart".to_string()), CborValue::Integer(0));
        index_map.insert(CborValue::Text("byteEnd".to_string()), CborValue::Integer(10));
        facet_map.insert(CborValue::Text("index".to_string()), CborValue::Map(index_map));
        
        // Create features with link
        let mut feature_map = BTreeMap::new();
        feature_map.insert(CborValue::Text("$type".to_string()), CborValue::Text("app.bsky.richtext.facet#link".to_string()));
        feature_map.insert(CborValue::Text("uri".to_string()), CborValue::Text("https://example.com".to_string()));
        
        let features = vec![CborValue::Map(feature_map)];
        facet_map.insert(CborValue::Text("features".to_string()), CborValue::Array(features));
        
        let facets_array = vec![CborValue::Map(facet_map)];
        
        let mut root_map = BTreeMap::new();
        root_map.insert(CborValue::Text("facets".to_string()), CborValue::Array(facets_array));
        
        let facets = parse_facets(&root_map);
        assert_eq!(facets.len(), 1);
        
        let facet = &facets[0];
        assert_eq!(facet.index.byte_start, 0);
        assert_eq!(facet.index.byte_end, 10);
        assert_eq!(facet.features.len(), 1);
        
        match &facet.features[0] {
            FacetFeature::Link { uri } => {
                assert_eq!(uri, "https://example.com");
            }
            _ => panic!("Expected Link feature"),
        }
    }

    #[tokio::test]
    async fn test_car_processor_creation() {
        let processor = CarProcessor::new();
        assert!(processor.is_ok());
    }

    #[test]
    fn test_find_posts_in_car_empty() {
        let processor = CarProcessor::new().unwrap();
        
        // Create minimal CAR with just header, no blocks
        let car_data = create_test_car_data(vec![]);
        
        let posts = processor.find_posts_in_car(&car_data).unwrap();
        assert_eq!(posts.len(), 0);
    }

    #[test]
    fn test_find_posts_in_car_with_post() {
        let processor = CarProcessor::new().unwrap();
        
        // Create CBOR for a post record
        let mut post_map = BTreeMap::new();
        post_map.insert(CborValue::Text("$type".to_string()), CborValue::Text("app.bsky.feed.post".to_string()));
        post_map.insert(CborValue::Text("text".to_string()), CborValue::Text("Hello world!".to_string()));
        post_map.insert(CborValue::Text("createdAt".to_string()), CborValue::Text("2024-01-01T00:00:00Z".to_string()));
        
        let cbor_data = serde_cbor::to_vec(&CborValue::Map(post_map)).unwrap();
        
        // Create CAR with this block
        let car_data = create_test_car_data(vec![
            ("0170122012345678901234567890123456789012345678901234567890123456", &cbor_data)
        ]);
        
        let posts = processor.find_posts_in_car(&car_data).unwrap();
        assert_eq!(posts.len(), 1);
        assert_eq!(posts[0].text, "Hello world!");
        assert_eq!(posts[0].created_at, "2024-01-01T00:00:00Z");
    }

    #[test]
    fn test_find_profile_in_car_with_profile() {
        let processor = CarProcessor::new().unwrap();
        
        // Create CBOR for a profile record
        let mut profile_map = BTreeMap::new();
        profile_map.insert(CborValue::Text("$type".to_string()), CborValue::Text("app.bsky.actor.profile".to_string()));
        profile_map.insert(CborValue::Text("displayName".to_string()), CborValue::Text("Test User".to_string()));
        profile_map.insert(CborValue::Text("description".to_string()), CborValue::Text("Test bio".to_string()));
        profile_map.insert(CborValue::Text("createdAt".to_string()), CborValue::Text("2024-01-01T00:00:00Z".to_string()));
        
        let cbor_data = serde_cbor::to_vec(&CborValue::Map(profile_map)).unwrap();
        
        let car_data = create_test_car_data(vec![
            ("0170122012345678901234567890123456789012345678901234567890123456", &cbor_data)
        ]);
        
        let profile = processor.find_profile_in_car(&car_data).unwrap();
        assert!(profile.is_some());
        
        let profile = profile.unwrap();
        assert_eq!(profile.display_name, Some("Test User".to_string()));
        assert_eq!(profile.description, Some("Test bio".to_string()));
        assert_eq!(profile.created_at, "2024-01-01T00:00:00Z");
    }

    #[test]
    fn test_car_parse_malformed_data() {
        let processor = CarProcessor::new().unwrap();
        
        // Test with completely invalid data
        let result = processor.find_posts_in_car(&[0xFF, 0xFF, 0xFF]);
        assert!(result.is_err());
        
        // Test with truncated header
        let result = processor.find_posts_in_car(&[0x10]); // header length 16 but no data
        assert!(result.is_err());
        
        // Test with empty data
        let result = processor.find_posts_in_car(&[]);
        assert!(result.is_err());
    }
}