/// Shared utility functions for the core layer.

/// FNV-1a hash for byte slices.
/// Used for fast content-addressable deduplication of OCR frames and text.
///
/// FNV-1a properties:
/// - Deterministic: same input always produces same hash
/// - Fast: single pass, no allocations
/// - Reasonable collision resistance for frame comparison use cases
#[inline]
pub fn fnv_hash_bytes(data: &[u8]) -> u64 {
    const FNV_OFFSET: u64 = 0xcbf29ce484222325;
    const FNV_PRIME: u64 = 0x00000100000001B3;
    let mut hash = FNV_OFFSET;
    for &byte in data {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}

/// FNV-1a hash for string slices (convenience wrapper).
#[inline]
pub fn fnv_hash_str(s: &str) -> u64 {
    fnv_hash_bytes(s.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_deterministic() {
        let data = b"hello world";
        assert_eq!(fnv_hash_bytes(data), fnv_hash_bytes(data));
    }

    #[test]
    fn hash_different_inputs_differ() {
        assert_ne!(fnv_hash_bytes(b"hello"), fnv_hash_bytes(b"world"));
    }

    #[test]
    fn hash_empty_returns_offset_basis() {
        assert_eq!(fnv_hash_bytes(b""), 0xcbf29ce484222325);
    }

    #[test]
    fn hash_str_matches_bytes() {
        let s = "テスト";
        assert_eq!(fnv_hash_str(s), fnv_hash_bytes(s.as_bytes()));
    }

    #[test]
    fn hash_single_byte_avalanche() {
        let h1 = fnv_hash_bytes(b"a");
        let h2 = fnv_hash_bytes(b"b");
        assert_ne!(h1, h2);
        let diff_bits = (h1 ^ h2).count_ones();
        assert!(diff_bits > 8, "Expected significant avalanche, got {} bit differences", diff_bits);
    }

    #[test]
    fn hash_known_value() {
        // FNV-1a("") = offset basis
        // FNV-1a("a") should match the known reference value
        let h = fnv_hash_bytes(b"a");
        assert_eq!(h, 0xaf63dc4c8601ec8c);
    }
}
