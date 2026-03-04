use anyhow::{Context, Result, anyhow};
use reqwest::StatusCode;
use reqwest::header::{ACCEPT, AUTHORIZATION, HeaderMap, HeaderValue, USER_AGENT};

use crate::models::release::{GitHubReleaseResponse, Release};

const DEFAULT_PREVIA_REPO: &str = "cloudvibedev/previa";

#[derive(Clone, Debug)]
pub struct GitHubClient {
    client: reqwest::Client,
    latest_release_url: String,
}

impl GitHubClient {
    pub fn new() -> Result<Self> {
        let repo = std::env::var("PREVIA_REPO").unwrap_or_else(|_| DEFAULT_PREVIA_REPO.to_string());
        let latest_release_url = format!("https://api.github.com/repos/{repo}/releases/latest");

        let mut headers = HeaderMap::new();
        headers.insert(USER_AGENT, HeaderValue::from_static("previactl/0.0.2"));
        headers.insert(
            ACCEPT,
            HeaderValue::from_static("application/vnd.github+json"),
        );

        if let Ok(token) = std::env::var("GITHUB_TOKEN") {
            let value = format!("Bearer {token}");
            headers.insert(
                AUTHORIZATION,
                HeaderValue::from_str(&value).context("GITHUB_TOKEN invalido")?,
            );
        }

        let client = reqwest::Client::builder()
            .default_headers(headers)
            .build()
            .context("falha ao inicializar cliente HTTP")?;

        Ok(Self {
            client,
            latest_release_url,
        })
    }

    pub async fn latest_release(&self) -> Result<Release> {
        let response = self
            .client
            .get(&self.latest_release_url)
            .send()
            .await
            .context("falha ao consultar release mais recente")?;

        if response.status() == StatusCode::NOT_FOUND {
            return Err(anyhow!(
                "release mais recente nao encontrada (404). Verifique se existe uma release publicada ou se PREVIA_REPO esta correto."
            ));
        }

        if !response.status().is_success() {
            return Err(anyhow!(
                "GitHub API retornou status {} ao buscar release mais recente",
                response.status()
            ));
        }

        let release: GitHubReleaseResponse = response
            .json()
            .await
            .context("falha ao parsear payload de release")?;

        Ok(release.into())
    }

    pub async fn download_to_path(&self, url: &str, destination: &std::path::Path) -> Result<()> {
        let response = self
            .client
            .get(url)
            .send()
            .await
            .with_context(|| format!("falha ao baixar asset: {url}"))?;

        if !response.status().is_success() {
            return Err(anyhow!(
                "download falhou com status {} para {}",
                response.status(),
                url
            ));
        }

        let bytes = response
            .bytes()
            .await
            .with_context(|| format!("falha ao ler bytes do asset: {url}"))?;

        std::fs::write(destination, &bytes).with_context(|| {
            format!("falha ao escrever arquivo temporario em {:?}", destination)
        })?;

        Ok(())
    }
}
