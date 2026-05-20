//! Shared utility functions for the core layer.

/// Returns the current time in milliseconds since the UNIX epoch.
#[inline]
pub fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

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

/// Smart hash converts RGBA to thresholded grayscale before hashing.
/// This prevents minor lighting/background particle changes from triggering text translation.
/// Uses FNV-1a internally (see `crate::core::utils::fnv_hash_bytes` for the plain variant).
pub fn smart_hash(data: &[u8]) -> u64 {
    const FNV_OFFSET: u64 = 0xcbf29ce484222325;
    const FNV_PRIME: u64 = 0x00000100000001B3;
    let mut h: u64 = FNV_OFFSET;

    // Dynamic step: smaller regions need finer sampling to detect
    // single-character text changes; large regions can skip more.
    let pixel_count = data.len() / 4;
    let step: usize = if pixel_count < 50_000 { 8 } else { 32 };
    let mut i = 0;
    while i + 2 < data.len() {
        // Quantize each channel to 3 bits (8 levels) to ignore compression noise and dithering
        let r = data[i] >> 5;
        let g = data[i + 1] >> 5;
        let b = data[i + 2] >> 5;
        let combined = (r << 6) | (g << 3) | b;

        h ^= combined as u64;
        h = h.wrapping_mul(FNV_PRIME);

        i += step;
    }
    h
}

/// Enforces cache size limit by removing oldest entries if cache is full
pub fn enforce_cache_limit<K, V>(
    cache: &mut parking_lot::MutexGuard<indexmap::IndexMap<K, V>>,
    max_entries: usize,
) where
    K: Clone + std::hash::Hash + std::cmp::Eq,
{
    if cache.len() >= max_entries {
        // Remove oldest entries (true FIFO by removing first few elements)
        let to_remove = cache.len() - max_entries + 1; // +1 to make room for new entry
        for _ in 0..to_remove {
            cache.shift_remove_index(0);
        }
        tracing::warn!("Cache limit reached, removed {} oldest entries", to_remove);
    }
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
        assert!(
            diff_bits > 8,
            "Expected significant avalanche, got {} bit differences",
            diff_bits
        );
    }

    #[test]
    fn hash_known_value() {
        // FNV-1a("") = offset basis
        // FNV-1a("a") should match the known reference value
        let h = fnv_hash_bytes(b"a");
        assert_eq!(h, 0xaf63dc4c8601ec8c);
    }
}
