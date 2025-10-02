use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone)]
pub struct Cid {
    #[allow(dead_code)] // Part of IPFS/AT Protocol spec - keep for future MCP tools
    pub version: u8,
    #[allow(dead_code)] // Part of IPFS/AT Protocol spec - keep for future MCP tools  
    pub codec: u8,
    #[allow(dead_code)] // Part of IPFS/AT Protocol spec - keep for future MCP tools
    pub digest_type: u8,
    #[allow(dead_code)] // Part of IPFS/AT Protocol spec - keep for future MCP tools
    pub digest: Vec<u8>,
}

impl fmt::Display for Cid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Use hex encoding of the digest for internal HashMap keys
        write!(f, "{}", hex::encode(&self.digest))
    }
}

impl Cid {
    
}

#[derive(Debug)]
pub struct CarEntry {
  #[allow(dead_code)] // not 100% sure may be removed later
    pub cid: Cid,
    pub bytes: Vec<u8>,
    // Remove unused positioning fields - these are just parsing artifacts
}

#[derive(Debug, Clone)]
pub struct CarHeader {
    #[allow(dead_code)] // Part of CAR format spec - keep for future MCP tools
    pub version: u8,
    #[allow(dead_code)] // Part of CAR format spec - keep for future MCP tools
    pub roots: Vec<Cid>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CidLink(pub String);

impl From<&Cid> for CidLink {
    fn from(cid: &Cid) -> Self {
        CidLink(cid.to_string())
    }
}

impl CidLink {
    #[allow(dead_code)] // Used in AT Protocol records - keep for future MCP tools
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cid_to_string() {
        let cid = Cid {
            version: 1,
            codec: 0x71,
            digest_type: 0x12,
            digest: vec![0x01, 0x02, 0x03, 0x04, 0x05],
        };
        
        let cid_string = cid.to_string();
        assert_eq!(cid_string, "0102030405");
    }

    #[test]
    fn test_cid_to_string_empty_digest() {
        let cid = Cid {
            version: 1,
            codec: 0x71,
            digest_type: 0x12,
            digest: vec![],
        };
        
        let cid_string = cid.to_string();
        assert_eq!(cid_string, "");
    }

    #[test]
    fn test_cid_clone() {
        let original = Cid {
            version: 1,
            codec: 0x71,
            digest_type: 0x12,
            digest: vec![0xAA, 0xBB, 0xCC],
        };
        
        let cloned = original.clone();
        assert_eq!(cloned.version, original.version);
        assert_eq!(cloned.codec, original.codec);
        assert_eq!(cloned.digest_type, original.digest_type);
        assert_eq!(cloned.digest, original.digest);
    }

    #[test]
    fn test_car_entry_structure() {
        let cid = Cid {
            version: 1,
            codec: 0x71,
            digest_type: 0x12,
            digest: vec![0x01, 0x02, 0x03],
        };
        
        let entry = CarEntry {
            cid: cid.clone(),
            bytes: vec![0x04, 0x05, 0x06],
        };
        
        assert_eq!(entry.cid.version, 1);
        assert_eq!(entry.bytes, vec![0x04, 0x05, 0x06]);
    }

    #[test]
    fn test_car_header_structure() {
        let root_cid = Cid {
            version: 1,
            codec: 0x71,
            digest_type: 0x12,
            digest: vec![0x01, 0x02, 0x03],
        };
        
        let header = CarHeader {
            version: 1,
            roots: vec![root_cid.clone()],
        };
        
        assert_eq!(header.version, 1);
        assert_eq!(header.roots.len(), 1);
        assert_eq!(header.roots[0].digest, vec![0x01, 0x02, 0x03]);
    }

    #[test]
    fn test_cid_link_from_cid() {
        let cid = Cid {
            version: 1,
            codec: 0x71,
            digest_type: 0x12,
            digest: vec![0xFF, 0xEE, 0xDD],
        };
        
        let link = CidLink::from(&cid);
        assert_eq!(link.0, "ffeedd");
        assert_eq!(link.as_str(), "ffeedd");
    }

    #[test]
    fn test_cid_link_serialization() {
        let link = CidLink("test123".to_string());
        
        // Test serialization
        let serialized = serde_json::to_string(&link).unwrap();
        assert_eq!(serialized, "\"test123\"");
        
        // Test deserialization
        let deserialized: CidLink = serde_json::from_str(&serialized).unwrap();
        assert_eq!(deserialized.0, "test123");
        assert_eq!(deserialized.as_str(), "test123");
    }

    #[test]
    fn test_car_header_clone() {
        let cid1 = Cid {
            version: 1,
            codec: 0x71,
            digest_type: 0x12,
            digest: vec![0x01, 0x02],
        };
        
        let cid2 = Cid {
            version: 1,
            codec: 0x55,
            digest_type: 0x12,
            digest: vec![0x03, 0x04],
        };
        
        let original = CarHeader {
            version: 1,
            roots: vec![cid1, cid2],
        };
        
        let cloned = original.clone();
        assert_eq!(cloned.version, original.version);
        assert_eq!(cloned.roots.len(), original.roots.len());
        assert_eq!(cloned.roots[0].digest, original.roots[0].digest);
        assert_eq!(cloned.roots[1].digest, original.roots[1].digest);
    }
}