use std::collections::{HashMap, HashSet};

use crate::server::models::SpecUrlEntry;

pub fn normalize_spec_slug(slug: Option<&str>) -> Result<Option<String>, &'static str> {
    let Some(raw) = slug else {
        return Ok(None);
    };

    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }

    let mut has_dash = false;
    let mut has_underscore = false;

    for ch in trimmed.chars() {
        if ch.is_ascii_lowercase() || ch.is_ascii_digit() {
            continue;
        }

        match ch {
            '-' => has_dash = true,
            '_' => has_underscore = true,
            _ => {
                return Err("slug must use only lowercase letters, numbers, '-' or '_'");
            }
        }
    }

    if has_dash && has_underscore {
        return Err("slug must be either dash-case or snake_case, not both");
    }

    let starts_or_ends_with_separator = trimmed.starts_with('-')
        || trimmed.ends_with('-')
        || trimmed.starts_with('_')
        || trimmed.ends_with('_');
    if starts_or_ends_with_separator {
        return Err("slug cannot start or end with '-' or '_'");
    }

    if trimmed.contains("--")
        || trimmed.contains("__")
        || trimmed.contains("-_")
        || trimmed.contains("_-")
    {
        return Err("slug cannot contain repeated or mixed adjacent separators");
    }

    Ok(Some(trimmed.to_owned()))
}

pub fn normalize_spec_urls(urls: Vec<SpecUrlEntry>) -> Result<Vec<SpecUrlEntry>, &'static str> {
    let mut normalized = Vec::with_capacity(urls.len());
    let mut seen = HashSet::new();

    for item in urls {
        let name = item.name.trim().to_ascii_lowercase();
        if name.is_empty() {
            return Err("spec urls[].name is required");
        }

        if !is_valid_spec_url_name(&name) {
            return Err("spec urls[].name must use only lowercase letters, numbers, '-' or '_'");
        }

        if !seen.insert(name.clone()) {
            return Err("spec urls[].name must be unique");
        }

        let url = item.url.trim().to_owned();
        if url.is_empty() {
            return Err("spec urls[].url is required");
        }

        let description = item
            .description
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_owned);

        normalized.push(SpecUrlEntry {
            name,
            url,
            description,
        });
    }

    Ok(normalized)
}

pub fn normalize_spec_urls_with_legacy(
    mut urls: Vec<SpecUrlEntry>,
    legacy_servers: HashMap<String, String>,
) -> Result<Vec<SpecUrlEntry>, &'static str> {
    if urls.is_empty() && !legacy_servers.is_empty() {
        let mut names: Vec<String> = legacy_servers.keys().cloned().collect();
        names.sort();
        for name in names {
            if let Some(url) = legacy_servers.get(&name) {
                urls.push(SpecUrlEntry {
                    name,
                    url: url.clone(),
                    description: None,
                });
            }
        }
    }

    normalize_spec_urls(urls)
}

pub fn build_servers_from_urls(
    urls: &[SpecUrlEntry],
    legacy_url: Option<&str>,
) -> HashMap<String, String> {
    let mut servers = HashMap::new();
    for item in urls {
        if item.name.trim().is_empty() || item.url.trim().is_empty() {
            continue;
        }
        servers.insert(item.name.clone(), item.url.clone());
    }

    if servers.is_empty() {
        if let Some(url) = legacy_url.map(str::trim).filter(|value| !value.is_empty()) {
            servers.insert("default".to_owned(), url.to_owned());
        }
    }

    servers
}

pub fn is_valid_spec_url_name(name: &str) -> bool {
    if name.starts_with('-')
        || name.ends_with('-')
        || name.starts_with('_')
        || name.ends_with('_')
        || name.contains("--")
        || name.contains("__")
        || name.contains("-_")
        || name.contains("_-")
    {
        return false;
    }

    name.chars()
        .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '-' || ch == '_')
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use crate::server::models::SpecUrlEntry;
    use crate::server::validation::specs::{
        normalize_spec_slug, normalize_spec_urls, normalize_spec_urls_with_legacy,
    };

    #[test]
    fn accepts_dashcase_slug() {
        let slug = normalize_spec_slug(Some("payments-api-v2")).expect("dashcase should be valid");
        assert_eq!(slug.as_deref(), Some("payments-api-v2"));
    }

    #[test]
    fn accepts_snakecase_slug() {
        let slug = normalize_spec_slug(Some("payments_api_v2")).expect("snakecase should be valid");
        assert_eq!(slug.as_deref(), Some("payments_api_v2"));
    }

    #[test]
    fn rejects_mixed_separator_slug() {
        let err = normalize_spec_slug(Some("payments-api_v2"))
            .expect_err("mixed separators should be invalid");
        assert!(err.contains("either dash-case or snake_case"));
    }

    #[test]
    fn rejects_uppercase_slug() {
        let err =
            normalize_spec_slug(Some("Payments-api")).expect_err("uppercase should be invalid");
        assert!(err.contains("lowercase letters"));
    }

    #[test]
    fn normalizes_empty_slug_to_none() {
        let slug = normalize_spec_slug(Some("   ")).expect("empty after trim should be none");
        assert_eq!(slug, None);
    }

    #[test]
    fn normalizes_spec_urls_payload() {
        let urls = normalize_spec_urls(vec![SpecUrlEntry {
            name: " HML ".to_owned(),
            url: " https://api.example.com ".to_owned(),
            description: Some("  ambiente ".to_owned()),
        }])
        .expect("spec urls should be valid");

        assert_eq!(urls.len(), 1);
        assert_eq!(urls[0].name, "hml");
        assert_eq!(urls[0].url, "https://api.example.com");
        assert_eq!(urls[0].description.as_deref(), Some("ambiente"));
    }

    #[test]
    fn rejects_invalid_spec_url_name() {
        let err = normalize_spec_urls(vec![SpecUrlEntry {
            name: "hml prod".to_owned(),
            url: "https://api.example.com".to_owned(),
            description: None,
        }])
        .expect_err("invalid name should be rejected");

        assert!(err.contains("spec urls[].name"));
    }

    #[test]
    fn rejects_duplicate_spec_url_name() {
        let err = normalize_spec_urls(vec![
            SpecUrlEntry {
                name: "hml".to_owned(),
                url: "https://hml.example.com".to_owned(),
                description: None,
            },
            SpecUrlEntry {
                name: "HML".to_owned(),
                url: "https://hml2.example.com".to_owned(),
                description: None,
            },
        ])
        .expect_err("duplicate names should be rejected");

        assert!(err.contains("must be unique"));
    }

    #[test]
    fn supports_legacy_servers_payload() {
        let urls = normalize_spec_urls_with_legacy(
            Vec::new(),
            HashMap::from([
                ("hml".to_owned(), "https://hml.example.com".to_owned()),
                ("prd".to_owned(), "https://api.example.com".to_owned()),
            ]),
        )
        .expect("legacy servers should be accepted");

        assert_eq!(urls.len(), 2);
        assert!(urls.iter().any(|item| item.name == "hml"));
        assert!(urls.iter().any(|item| item.name == "prd"));
    }
}
