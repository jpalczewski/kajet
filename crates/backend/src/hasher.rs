use xxhash_rust::xxh3::xxh3_64;

pub fn hash_content(content: &str) -> String {
    format!("{:016x}", xxh3_64(content.as_bytes()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_is_deterministic() {
        let h1 = hash_content("hello world");
        let h2 = hash_content("hello world");
        assert_eq!(h1, h2);
    }

    #[test]
    fn different_content_different_hash() {
        let h1 = hash_content("hello");
        let h2 = hash_content("world");
        assert_ne!(h1, h2);
    }

    #[test]
    fn hash_is_16_chars_hex() {
        let h = hash_content("test");
        assert_eq!(h.len(), 16);
        assert!(h.chars().all(|c| c.is_ascii_hexdigit()));
    }
}
