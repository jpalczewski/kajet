pub fn apply_prefix(prefix: &str, text: &str) -> String {
    if prefix.is_empty() {
        return text.to_string();
    }
    format!("{prefix}{text}")
}
