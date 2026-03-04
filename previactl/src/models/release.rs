use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct GitHubReleaseResponse {
    pub tag_name: String,
    pub assets: Vec<GitHubAssetResponse>,
}

#[derive(Debug, Deserialize)]
pub struct GitHubAssetResponse {
    pub name: String,
    pub browser_download_url: String,
}

#[derive(Clone, Debug)]
pub struct Release {
    pub tag: String,
    pub assets: Vec<ReleaseAsset>,
}

#[derive(Clone, Debug)]
pub struct ReleaseAsset {
    pub name: String,
    pub download_url: String,
}

impl From<GitHubReleaseResponse> for Release {
    fn from(value: GitHubReleaseResponse) -> Self {
        let assets = value
            .assets
            .into_iter()
            .map(|asset| ReleaseAsset {
                name: asset.name,
                download_url: asset.browser_download_url,
            })
            .collect();

        Self {
            tag: value.tag_name,
            assets,
        }
    }
}
