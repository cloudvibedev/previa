pub(crate) fn parse_method(method: &str) -> Result<reqwest::Method, String> {
    reqwest::Method::from_bytes(method.as_bytes())
        .map_err(|_| format!("invalid HTTP method: {}", method))
}

pub(crate) fn parse_absolute_http_url(url: &str) -> Result<reqwest::Url, String> {
    let parsed = reqwest::Url::parse(url)
        .map_err(|_| format!("step url must be absolute (http/https): {}", url))?;
    match parsed.scheme() {
        "http" | "https" => Ok(parsed),
        _ => Err(format!("step url must be absolute (http/https): {}", url)),
    }
}
