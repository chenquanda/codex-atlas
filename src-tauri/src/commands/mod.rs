#[tauri::command]
pub fn health() -> &'static str {
    "Codex Atlas"
}

#[cfg(test)]
mod tests {
    use super::health;

    #[test]
    fn health_returns_product_name() {
        assert_eq!(health(), "Codex Atlas");
    }
}
