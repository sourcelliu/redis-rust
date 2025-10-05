// Hash slot management

/// Number of hash slots in Redis Cluster
pub const CLUSTER_SLOTS: u16 = 16384;

/// CRC16 implementation for Redis Cluster
/// This is the XMODEM variant of CRC16
pub fn crc16(key: &[u8]) -> u16 {
    const CRC16TAB: [u16; 256] = [
        0x0000, 0x1021, 0x2042, 0x3063, 0x4084, 0x50a5, 0x60c6, 0x70e7,
        0x8108, 0x9129, 0xa14a, 0xb16b, 0xc18c, 0xd1ad, 0xe1ce, 0xf1ef,
        0x1231, 0x0210, 0x3273, 0x2252, 0x52b5, 0x4294, 0x72f7, 0x62d6,
        0x9339, 0x8318, 0xb37b, 0xa35a, 0xd3bd, 0xc39c, 0xf3ff, 0xe3de,
        0x2462, 0x3443, 0x0420, 0x1401, 0x64e6, 0x74c7, 0x44a4, 0x5485,
        0xa56a, 0xb54b, 0x8528, 0x9509, 0xe5ee, 0xf5cf, 0xc5ac, 0xd58d,
        0x3653, 0x2672, 0x1611, 0x0630, 0x76d7, 0x66f6, 0x5695, 0x46b4,
        0xb75b, 0xa77a, 0x9719, 0x8738, 0xf7df, 0xe7fe, 0xd79d, 0xc7bc,
        0x48c4, 0x58e5, 0x6886, 0x78a7, 0x0840, 0x1861, 0x2802, 0x3823,
        0xc9cc, 0xd9ed, 0xe98e, 0xf9af, 0x8948, 0x9969, 0xa90a, 0xb92b,
        0x5af5, 0x4ad4, 0x7ab7, 0x6a96, 0x1a71, 0x0a50, 0x3a33, 0x2a12,
        0xdbfd, 0xcbdc, 0xfbbf, 0xeb9e, 0x9b79, 0x8b58, 0xbb3b, 0xab1a,
        0x6ca6, 0x7c87, 0x4ce4, 0x5cc5, 0x2c22, 0x3c03, 0x0c60, 0x1c41,
        0xedae, 0xfd8f, 0xcdec, 0xddcd, 0xad2a, 0xbd0b, 0x8d68, 0x9d49,
        0x7e97, 0x6eb6, 0x5ed5, 0x4ef4, 0x3e13, 0x2e32, 0x1e51, 0x0e70,
        0xff9f, 0xefbe, 0xdfdd, 0xcffc, 0xbf1b, 0xaf3a, 0x9f59, 0x8f78,
        0x9188, 0x81a9, 0xb1ca, 0xa1eb, 0xd10c, 0xc12d, 0xf14e, 0xe16f,
        0x1080, 0x00a1, 0x30c2, 0x20e3, 0x5004, 0x4025, 0x7046, 0x6067,
        0x83b9, 0x9398, 0xa3fb, 0xb3da, 0xc33d, 0xd31c, 0xe37f, 0xf35e,
        0x02b1, 0x1290, 0x22f3, 0x32d2, 0x4235, 0x5214, 0x6277, 0x7256,
        0xb5ea, 0xa5cb, 0x95a8, 0x8589, 0xf56e, 0xe54f, 0xd52c, 0xc50d,
        0x34e2, 0x24c3, 0x14a0, 0x0481, 0x7466, 0x6447, 0x5424, 0x4405,
        0xa7db, 0xb7fa, 0x8799, 0x97b8, 0xe75f, 0xf77e, 0xc71d, 0xd73c,
        0x26d3, 0x36f2, 0x0691, 0x16b0, 0x6657, 0x7676, 0x4615, 0x5634,
        0xd94c, 0xc96d, 0xf90e, 0xe92f, 0x99c8, 0x89e9, 0xb98a, 0xa9ab,
        0x5844, 0x4865, 0x7806, 0x6827, 0x18c0, 0x08e1, 0x3882, 0x28a3,
        0xcb7d, 0xdb5c, 0xeb3f, 0xfb1e, 0x8bf9, 0x9bd8, 0xabbb, 0xbb9a,
        0x4a75, 0x5a54, 0x6a37, 0x7a16, 0x0af1, 0x1ad0, 0x2ab3, 0x3a92,
        0xfd2e, 0xed0f, 0xdd6c, 0xcd4d, 0xbdaa, 0xad8b, 0x9de8, 0x8dc9,
        0x7c26, 0x6c07, 0x5c64, 0x4c45, 0x3ca2, 0x2c83, 0x1ce0, 0x0cc1,
        0xef1f, 0xff3e, 0xcf5d, 0xdf7c, 0xaf9b, 0xbfba, 0x8fd9, 0x9ff8,
        0x6e17, 0x7e36, 0x4e55, 0x5e74, 0x2e93, 0x3eb2, 0x0ed1, 0x1ef0,
    ];

    let mut crc: u16 = 0;
    for &byte in key {
        let idx = ((crc >> 8) as u8 ^ byte) as usize;
        crc = (crc << 8) ^ CRC16TAB[idx];
    }
    crc
}

/// Extract hash tag from key if present
/// Hash tags are enclosed in curly braces: {tag}
/// Example: {user}:profile -> "user"
pub fn extract_hash_tag(key: &[u8]) -> &[u8] {
    // Find first '{'
    if let Some(start) = key.iter().position(|&b| b == b'{') {
        // Find first '}' after '{'
        if let Some(end) = key[start + 1..].iter().position(|&b| b == b'}') {
            let tag_end = start + 1 + end;
            // Return the content between braces if not empty
            if tag_end > start + 1 {
                return &key[start + 1..tag_end];
            }
        }
    }
    // No valid hash tag found, return whole key
    key
}

/// Calculate hash slot for a key
/// Redis uses CRC16(key) mod 16384
pub fn key_hash_slot(key: &[u8]) -> u16 {
    let hash_key = extract_hash_tag(key);
    crc16(hash_key) % CLUSTER_SLOTS
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_crc16_basic() {
        // Test vectors from Redis
        assert_eq!(crc16(b"123456789"), 0x31C3);
        assert_eq!(crc16(b""), 0x0000);
        assert_eq!(crc16(b"a"), 0x3FBD);
    }

    #[test]
    fn test_extract_hash_tag() {
        // Valid hash tags
        assert_eq!(extract_hash_tag(b"{user}:profile"), b"user");
        assert_eq!(extract_hash_tag(b"{user}"), b"user");
        assert_eq!(extract_hash_tag(b"prefix:{tag}:suffix"), b"tag");
        assert_eq!(extract_hash_tag(b"{a}{b}"), b"a"); // First tag only

        // Invalid or no hash tags
        assert_eq!(extract_hash_tag(b"no_tag"), b"no_tag");
        assert_eq!(extract_hash_tag(b"{}key"), b"{}key"); // Empty tag ignored
        assert_eq!(extract_hash_tag(b"{incomplete"), b"{incomplete");
        assert_eq!(extract_hash_tag(b"reverse}"), b"reverse}");
    }

    #[test]
    fn test_key_hash_slot() {
        // Same key should always produce same slot
        let key = b"mykey";
        let slot1 = key_hash_slot(key);
        let slot2 = key_hash_slot(key);
        assert_eq!(slot1, slot2);

        // Slot must be in valid range
        assert!(slot1 < CLUSTER_SLOTS);

        // Keys with same hash tag should map to same slot
        let key1 = b"{user}:profile";
        let key2 = b"{user}:settings";
        assert_eq!(key_hash_slot(key1), key_hash_slot(key2));

        // Different keys should (usually) map to different slots
        let key_a = b"keyA";
        let key_b = b"keyB";
        // This could theoretically be equal, but very unlikely
        // Just verify both are valid
        assert!(key_hash_slot(key_a) < CLUSTER_SLOTS);
        assert!(key_hash_slot(key_b) < CLUSTER_SLOTS);
    }

    #[test]
    fn test_slot_distribution() {
        // Test that slots are reasonably distributed
        let test_keys: Vec<Vec<u8>> = (0..1000)
            .map(|i| format!("key{}", i).into_bytes())
            .collect();

        let mut slot_counts = vec![0u32; CLUSTER_SLOTS as usize];
        for key in &test_keys {
            let slot = key_hash_slot(key);
            slot_counts[slot as usize] += 1;
        }

        // At least some different slots should be used
        let used_slots = slot_counts.iter().filter(|&&count| count > 0).count();
        assert!(used_slots > 100); // Reasonable distribution
    }

    #[test]
    fn test_cluster_slots_constant() {
        assert_eq!(CLUSTER_SLOTS, 16384);
    }
}
