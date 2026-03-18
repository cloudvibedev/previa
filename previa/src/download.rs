use std::collections::BTreeMap;
use std::env;
use std::io::{self, IsTerminal};
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{Context, Result, anyhow, bail};
use indicatif::{ProgressBar, ProgressDrawTarget, ProgressStyle};
use reqwest::Client;
use serde::Deserialize;
use tokio::fs;
use tokio::io::AsyncWriteExt;

use crate::paths::PreviaPaths;

const DEFAULT_MANIFEST_URL: &str = "https://downloads.previa.dev/latest.json";
const MANIFEST_URL_ENV: &str = "PREVIA_DOWNLOAD_MANIFEST_URL";

#[cfg(test)]
pub(crate) static MANIFEST_ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[derive(Debug, Deserialize)]
struct LatestManifest {
    version: String,
    #[serde(default)]
    links: BTreeMap<String, String>,
}

pub async fn ensure_runtime_binaries(paths: &PreviaPaths, local_runner_count: usize) -> Result<()> {
    let mut missing = Vec::new();
    if paths.main_binary().is_err() {
        missing.push("previa-main");
    }
    if local_runner_count > 0 && paths.runner_binary().is_err() {
        missing.push("previa-runner");
    }

    if missing.is_empty() {
        return Ok(());
    }

    let client = build_download_client()?;
    let manifest_url = manifest_url();
    let manifest = fetch_manifest(&client, &manifest_url).await?;

    for binary_name in missing {
        let mut reporter = DownloadReporter::for_stderr();
        download_binary(&client, paths, binary_name, &manifest, &mut reporter).await?;
    }

    Ok(())
}

fn build_download_client() -> Result<Client> {
    Client::builder()
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(120))
        .build()
        .context("failed to build binary download HTTP client")
}

fn manifest_url() -> String {
    env::var(MANIFEST_URL_ENV)
        .ok()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| DEFAULT_MANIFEST_URL.to_owned())
}

async fn fetch_manifest(client: &Client, url: &str) -> Result<LatestManifest> {
    let response = client
        .get(url)
        .send()
        .await
        .with_context(|| format!("failed to fetch manifest '{url}'"))?
        .error_for_status()
        .with_context(|| format!("failed to fetch manifest '{url}'"))?;

    response
        .json::<LatestManifest>()
        .await
        .with_context(|| format!("failed to parse manifest '{url}'"))
}

async fn download_binary(
    client: &Client,
    paths: &PreviaPaths,
    binary_name: &str,
    manifest: &LatestManifest,
    reporter: &mut impl ProgressReporter,
) -> Result<PathBuf> {
    let (os_slug, arch_slug) = normalized_platform()?;
    let manifest_key = manifest_key(binary_name, &os_slug, &arch_slug)?;
    let url = manifest.links.get(&manifest_key).ok_or_else(|| {
        anyhow!(
            "missing manifest link '{manifest_key}' for binary '{binary_name}' in '{}'",
            manifest_url()
        )
    })?;

    let target_path = binary_install_path(paths, binary_name);
    if target_path.exists() {
        return Ok(target_path);
    }

    if let Some(parent) = target_path.parent() {
        fs::create_dir_all(parent)
            .await
            .with_context(|| format!("failed to create '{}'", parent.display()))?;
    }

    let response = client
        .get(url)
        .send()
        .await
        .with_context(|| format!("failed to download binary '{binary_name}' from '{url}'"))?
        .error_for_status()
        .with_context(|| format!("failed to download binary '{binary_name}' from '{url}'"))?;

    reporter.begin(binary_name, &manifest.version, response.content_length());

    let temp_path = temporary_download_path(&target_path);
    let result =
        async {
            let mut file = fs::File::create(&temp_path).await.with_context(|| {
                format!(
                    "failed to create temporary file for binary '{}': '{}'",
                    binary_name,
                    temp_path.display()
                )
            })?;

            let mut response = response;
            while let Some(chunk) = response.chunk().await.with_context(|| {
                format!("failed to read download stream for binary '{binary_name}'")
            })? {
                file.write_all(&chunk).await.with_context(|| {
                    format!(
                        "failed to write downloaded bytes for binary '{}': '{}'",
                        binary_name,
                        temp_path.display()
                    )
                })?;
                reporter.advance(chunk.len() as u64);
            }

            file.flush().await.with_context(|| {
                format!(
                    "failed to flush temporary file for binary '{}': '{}'",
                    binary_name,
                    temp_path.display()
                )
            })?;
            drop(file);

            set_executable(&temp_path)?;
            fs::rename(&temp_path, &target_path)
                .await
                .with_context(|| {
                    format!(
                        "failed to install downloaded binary '{}': '{}' -> '{}'",
                        binary_name,
                        temp_path.display(),
                        target_path.display()
                    )
                })?;

            Result::<(), anyhow::Error>::Ok(())
        }
        .await;

    if result.is_err() {
        let _ = fs::remove_file(&temp_path).await;
    }

    result?;
    reporter.finish();
    Ok(target_path)
}

fn binary_install_path(paths: &PreviaPaths, binary_name: &str) -> PathBuf {
    paths.home.join("bin").join(binary_name)
}

fn temporary_download_path(target_path: &Path) -> PathBuf {
    let pid = std::process::id();
    target_path.with_extension(format!("download-{pid}.tmp"))
}

fn normalized_platform() -> Result<(String, String)> {
    let os_slug = match env::consts::OS {
        "linux" => "linux",
        other => {
            bail!(
                "unsupported operating system: {other}. Previa binaries are published for Linux only."
            )
        }
    };
    let arch_slug = match env::consts::ARCH {
        "x86_64" | "amd64" => "amd64",
        "aarch64" | "arm64" => "arm64",
        other => bail!("unsupported architecture: {other}."),
    };

    Ok((os_slug.to_owned(), arch_slug.to_owned()))
}

fn manifest_key(binary_name: &str, os_slug: &str, arch_slug: &str) -> Result<String> {
    let prefix = match binary_name {
        "previa-main" => "previa_main",
        "previa-runner" => "previa_runner",
        other => bail!("unsupported auto-download binary '{other}'"),
    };

    Ok(format!("{prefix}_{os_slug}_{arch_slug}"))
}

#[cfg(unix)]
fn set_executable(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;

    let mut permissions = std::fs::metadata(path)
        .with_context(|| format!("failed to read '{}'", path.display()))?
        .permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(path, permissions)
        .with_context(|| format!("failed to chmod '{}'", path.display()))
}

#[cfg(not(unix))]
fn set_executable(path: &Path) -> Result<()> {
    let metadata =
        std::fs::metadata(path).with_context(|| format!("failed to read '{}'", path.display()))?;
    let mut permissions = metadata.permissions();
    permissions.set_readonly(false);
    std::fs::set_permissions(path, permissions)
        .with_context(|| format!("failed to update permissions for '{}'", path.display()))
}

trait ProgressReporter {
    fn begin(&mut self, binary_name: &str, version: &str, total_bytes: Option<u64>);
    fn advance(&mut self, bytes: u64);
    fn finish(&mut self);
}

enum DownloadReporter {
    Visible(ProgressBar),
    Hidden,
}

impl DownloadReporter {
    fn for_stderr() -> Self {
        Self::for_terminal(io::stderr().is_terminal())
    }

    fn for_terminal(is_terminal: bool) -> Self {
        if is_terminal {
            let bar = ProgressBar::with_draw_target(None, ProgressDrawTarget::stderr_with_hz(10));
            Self::Visible(bar)
        } else {
            Self::Hidden
        }
    }

    #[cfg(test)]
    fn is_visible(&self) -> bool {
        matches!(self, Self::Visible(_))
    }
}

impl ProgressReporter for DownloadReporter {
    fn begin(&mut self, binary_name: &str, version: &str, total_bytes: Option<u64>) {
        let Self::Visible(bar) = self else {
            return;
        };

        let message = format!("Downloading {binary_name} {version}...");
        match total_bytes {
            Some(total) => {
                bar.set_length(total);
                let style = ProgressStyle::with_template(
                    "{spinner:.cyan} {msg} [{bar:40.cyan/blue}] {bytes}/{total_bytes}",
                )
                .expect("valid progress template")
                .progress_chars("#>-");
                bar.set_style(style);
            }
            None => {
                let style = ProgressStyle::with_template("{spinner:.cyan} {msg} {bytes}")
                    .expect("valid progress template");
                bar.set_style(style);
            }
        }
        bar.set_message(message);
        bar.enable_steady_tick(Duration::from_millis(100));
    }

    fn advance(&mut self, bytes: u64) {
        let Self::Visible(bar) = self else {
            return;
        };
        bar.inc(bytes);
    }

    fn finish(&mut self) {
        if let Self::Visible(bar) = self {
            bar.finish_and_clear();
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::env;

    use axum::extract::State;
    use axum::http::StatusCode;
    use axum::routing::get;
    use axum::{Json, Router};
    use serde_json::{Value, json};
    use tempfile::TempDir;
    use tokio::net::TcpListener;

    use super::{
        DownloadReporter, LatestManifest, MANIFEST_URL_ENV, binary_install_path, download_binary,
        fetch_manifest, manifest_key, normalized_platform,
    };
    use crate::paths::PreviaPaths;

    #[derive(Default)]
    struct RecordingReporter {
        started: bool,
        finished: bool,
        binary_name: Option<String>,
        version: Option<String>,
        total_bytes: Option<u64>,
        advanced: u64,
    }

    impl super::ProgressReporter for RecordingReporter {
        fn begin(&mut self, binary_name: &str, version: &str, total_bytes: Option<u64>) {
            self.started = true;
            self.binary_name = Some(binary_name.to_owned());
            self.version = Some(version.to_owned());
            self.total_bytes = total_bytes;
        }

        fn advance(&mut self, bytes: u64) {
            self.advanced += bytes;
        }

        fn finish(&mut self) {
            self.finished = true;
        }
    }

    #[derive(Clone)]
    struct TestServerState {
        manifest: Value,
        binaries: BTreeMap<String, Vec<u8>>,
        binary_status: BTreeMap<String, StatusCode>,
        requests: std::sync::Arc<std::sync::Mutex<Vec<String>>>,
    }

    async fn latest_manifest(State(state): State<TestServerState>) -> Json<Value> {
        state
            .requests
            .lock()
            .expect("requests lock")
            .push("/latest.json".to_owned());
        Json(state.manifest)
    }

    async fn binary_asset(
        State(state): State<TestServerState>,
        axum::extract::Path(name): axum::extract::Path<String>,
    ) -> (StatusCode, Vec<u8>) {
        state
            .requests
            .lock()
            .expect("requests lock")
            .push(format!("/files/{name}"));
        if let Some(status) = state.binary_status.get(&name) {
            return (*status, Vec::new());
        }
        match state.binaries.get(&name) {
            Some(bytes) => (StatusCode::OK, bytes.clone()),
            None => (StatusCode::NOT_FOUND, Vec::new()),
        }
    }

    async fn spawn_test_server(
        manifest: Value,
        binaries: BTreeMap<String, Vec<u8>>,
        binary_status: BTreeMap<String, StatusCode>,
    ) -> (String, std::sync::Arc<std::sync::Mutex<Vec<String>>>) {
        let requests = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let state = TestServerState {
            manifest,
            binaries,
            binary_status,
            requests: requests.clone(),
        };
        let app = Router::new()
            .route("/latest.json", get(latest_manifest))
            .route("/files/{name}", get(binary_asset))
            .with_state(state);

        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind listener");
        let address = listener.local_addr().expect("local addr");
        tokio::spawn(async move {
            axum::serve(listener, app).await.expect("serve app");
        });
        (format!("http://{address}"), requests)
    }

    fn temp_paths() -> (TempDir, PreviaPaths) {
        let temp = TempDir::new().expect("tempdir");
        let paths = PreviaPaths {
            home: temp.path().to_path_buf(),
            workspace_root: None,
        };
        (temp, paths)
    }

    #[test]
    fn manifest_key_resolves_expected_binary_names() {
        assert_eq!(
            manifest_key("previa-main", "linux", "amd64").expect("main key"),
            "previa_main_linux_amd64"
        );
        assert_eq!(
            manifest_key("previa-runner", "linux", "arm64").expect("runner key"),
            "previa_runner_linux_arm64"
        );
    }

    #[test]
    fn manifest_key_rejects_unsupported_binary_names() {
        let err = manifest_key("previa", "linux", "amd64").expect_err("invalid binary");
        assert!(err.to_string().contains("unsupported auto-download binary"));
    }

    #[test]
    fn platform_normalization_matches_supported_linux_targets() {
        let (os_slug, arch_slug) = normalized_platform().expect("platform");
        assert_eq!(os_slug, "linux");
        assert!(matches!(arch_slug.as_str(), "amd64" | "arm64"));
    }

    #[test]
    fn download_reporter_respects_terminal_visibility() {
        assert!(DownloadReporter::for_terminal(true).is_visible());
        assert!(!DownloadReporter::for_terminal(false).is_visible());
    }

    #[tokio::test]
    async fn fetch_manifest_ignores_unknown_fields() {
        let (base_url, _) = spawn_test_server(
            json!({
                "name": "previa",
                "version": "1.0.0-alpha.0",
                "create_at": "2026-03-18T21:23:56Z",
                "links": {
                    "previa_main_linux_amd64": "http://example.test/main"
                }
            }),
            BTreeMap::new(),
            BTreeMap::new(),
        )
        .await;

        let manifest = fetch_manifest(
            &super::build_download_client().expect("client"),
            &format!("{base_url}/latest.json"),
        )
        .await
        .expect("manifest");
        assert_eq!(manifest.version, "1.0.0-alpha.0");
        assert_eq!(
            manifest
                .links
                .get("previa_main_linux_amd64")
                .expect("manifest link"),
            "http://example.test/main"
        );
    }

    #[tokio::test]
    async fn downloads_missing_binary_from_manifest() {
        let (temp, paths) = temp_paths();
        let binary_name = "previa-main";
        let asset_name = "previa-main-linux-amd64";
        let payload = b"#!/bin/sh\necho downloaded\n".to_vec();
        let (base_url, requests) = spawn_test_server(
            json!({ "version": "1.0.0-alpha.0", "links": {} }),
            BTreeMap::from([(asset_name.to_owned(), payload.clone())]),
            BTreeMap::new(),
        )
        .await;

        let manifest = LatestManifest {
            version: "1.0.0-alpha.0".to_owned(),
            links: BTreeMap::from([(
                "previa_main_linux_amd64".to_owned(),
                format!("{base_url}/files/{asset_name}"),
            )]),
        };
        let mut reporter = RecordingReporter::default();
        let client = super::build_download_client().expect("client");

        let installed = download_binary(&client, &paths, binary_name, &manifest, &mut reporter)
            .await
            .expect("downloaded binary");

        assert_eq!(installed, binary_install_path(&paths, binary_name));
        assert_eq!(std::fs::read(&installed).expect("binary bytes"), payload);
        assert!(reporter.started);
        assert!(reporter.finished);
        assert_eq!(reporter.binary_name.as_deref(), Some(binary_name));
        assert_eq!(reporter.version.as_deref(), Some("1.0.0-alpha.0"));
        assert_eq!(reporter.total_bytes, Some(26));
        assert_eq!(reporter.advanced, 26);
        assert_eq!(
            requests.lock().expect("requests lock").as_slice(),
            &[format!("/files/{asset_name}")]
        );
        drop(temp);
    }

    #[tokio::test]
    async fn download_skips_when_local_binary_already_exists() {
        let (_temp, paths) = temp_paths();
        let install_path = binary_install_path(&paths, "previa-main");
        std::fs::create_dir_all(install_path.parent().expect("bin dir")).expect("bin dir");
        std::fs::write(&install_path, b"local").expect("local binary");

        let manifest = LatestManifest {
            version: "1.0.0-alpha.0".to_owned(),
            links: BTreeMap::from([(
                "previa_main_linux_amd64".to_owned(),
                "http://example.test/files/previa-main-linux-amd64".to_owned(),
            )]),
        };
        let mut reporter = RecordingReporter::default();

        let installed = download_binary(
            &super::build_download_client().expect("client"),
            &paths,
            "previa-main",
            &manifest,
            &mut reporter,
        )
        .await
        .expect("local binary");

        assert_eq!(installed, install_path);
        assert!(!reporter.started);
        assert!(!reporter.finished);
        assert_eq!(std::fs::read(&installed).expect("binary bytes"), b"local");
    }

    #[tokio::test]
    async fn download_fails_when_manifest_link_is_missing() {
        let (_temp, paths) = temp_paths();
        let manifest = LatestManifest {
            version: "1.0.0-alpha.0".to_owned(),
            links: BTreeMap::new(),
        };

        let err = download_binary(
            &super::build_download_client().expect("client"),
            &paths,
            "previa-main",
            &manifest,
            &mut RecordingReporter::default(),
        )
        .await
        .expect_err("missing link");

        assert!(
            err.to_string()
                .contains("missing manifest link 'previa_main_linux_amd64'")
        );
    }

    #[tokio::test]
    async fn download_fails_when_binary_download_fails() {
        let (_temp, paths) = temp_paths();
        let asset_name = "previa-main-linux-amd64";
        let (base_url, _) = spawn_test_server(
            json!({
                "version": "1.0.0-alpha.0",
                "links": {}
            }),
            BTreeMap::new(),
            BTreeMap::from([(asset_name.to_owned(), StatusCode::INTERNAL_SERVER_ERROR)]),
        )
        .await;
        let manifest = LatestManifest {
            version: "1.0.0-alpha.0".to_owned(),
            links: BTreeMap::from([(
                "previa_main_linux_amd64".to_owned(),
                format!("{base_url}/files/{asset_name}"),
            )]),
        };

        let err = download_binary(
            &super::build_download_client().expect("client"),
            &paths,
            "previa-main",
            &manifest,
            &mut RecordingReporter::default(),
        )
        .await
        .expect_err("download failure");

        assert!(
            err.to_string()
                .contains("failed to download binary 'previa-main'")
        );
    }

    #[tokio::test]
    async fn manifest_url_uses_environment_override() {
        let _guard = super::MANIFEST_ENV_LOCK.lock().expect("manifest env lock");
        let (base_url, _) = spawn_test_server(
            json!({ "version": "1.0.0-alpha.0", "links": {} }),
            BTreeMap::new(),
            BTreeMap::new(),
        )
        .await;
        unsafe {
            env::set_var(MANIFEST_URL_ENV, format!("{base_url}/latest.json"));
        }
        assert_eq!(super::manifest_url(), format!("{base_url}/latest.json"));
        unsafe {
            env::remove_var(MANIFEST_URL_ENV);
        }
    }
}
