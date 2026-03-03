pub(crate) fn parse_method(method: &str) -> Result<reqwest::Method, String> {
    reqwest::Method::from_bytes(method.as_bytes())
        .map_err(|_| format!("invalid HTTP method: {}", method))
}
