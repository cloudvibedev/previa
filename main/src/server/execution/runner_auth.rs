use reqwest::RequestBuilder;
use reqwest::header::AUTHORIZATION;

pub fn apply_runner_auth(request: RequestBuilder, runner_auth_key: Option<&str>) -> RequestBuilder {
    match runner_auth_key
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        Some(key) => request.header(AUTHORIZATION, key),
        None => request,
    }
}

#[cfg(test)]
mod tests {
    use reqwest::Client;

    use super::apply_runner_auth;

    #[test]
    fn injects_authorization_header_when_key_is_present() {
        let client = Client::new();
        let request = apply_runner_auth(client.get("http://localhost"), Some("secret"))
            .build()
            .expect("request");
        assert_eq!(
            request
                .headers()
                .get(reqwest::header::AUTHORIZATION)
                .and_then(|value| value.to_str().ok()),
            Some("secret")
        );
    }

    #[test]
    fn skips_authorization_header_when_key_is_absent() {
        let client = Client::new();
        let request = apply_runner_auth(client.get("http://localhost"), None)
            .build()
            .expect("request");
        assert!(
            request
                .headers()
                .get(reqwest::header::AUTHORIZATION)
                .is_none()
        );
    }
}
