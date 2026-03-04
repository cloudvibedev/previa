use anyhow::{Context, Result, anyhow};
use reqwest::StatusCode;
use reqwest::header::{ACCEPT, AUTHORIZATION, HeaderMap, HeaderValue, USER_AGENT};

use crate::models::release::{GitHubReleaseResponse, Release};

const PREVIA_REPO_URL: &str = "https://github.com/cloudvibedev/previa";
const PREVIA_RELEASES_URL: &str = "https://github.com/cloudvibedev/previa/releases";
const PREVIA_LATEST_RELEASE_API_URL: &str =
    "https://api.github.com/repos/cloudvibedev/previa/releases/latest";

#[derive(Clone, Debug)]
pub struct GitHubClient {
    client: reqwest::Client,
}

impl GitHubClient {
    pub fn new() -> Result<Self> {
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

        Ok(Self { client })
    }

    pub async fn latest_release(&self) -> Result<Release> {
        let response = self
            .client
            .get(PREVIA_LATEST_RELEASE_API_URL)
            .send()
            .await
            .context("falha ao consultar release mais recente")?;

        if response.status() == StatusCode::NOT_FOUND {
            return Err(anyhow!(
                "release mais recente nao encontrada (404) no repositorio principal: {PREVIA_REPO_URL}. Verifique se existe uma release publicada."
            ));
        }

        if !response.status().is_success() {
            return Err(anyhow!(
                "GitHub API retornou status {} ao buscar release mais recente. Consulte: {}",
                response.status(),
                PREVIA_RELEASES_URL
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
