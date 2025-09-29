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
use serde_json::Value as JsonValue;
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
    let ver = read_uvarint(block, &mut i)?;
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

    /// Clean up expired cache
    pub async fn cleanup_cache(&self) -> Result<(), AppError> {
        self.cache.cleanup_expired()
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

/// Compute length of first varint (helper for header handling)
fn last_read_uvarint_len(data: &[u8], start: usize) -> Option<usize> {
    let mut i = start;
    while i < data.len() {
        let b = data[i];
        i += 1;
        if b < 0x80 { return Some(i); }
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