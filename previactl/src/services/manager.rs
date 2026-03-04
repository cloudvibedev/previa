use std::cmp::Ordering;
use std::fs;
use std::fs::File;
use std::io;
use std::os::unix::fs::PermissionsExt;

use anyhow::{Context, Result, anyhow};
use dialoguer::Confirm;
use semver::Version;
use tar::Archive;
use tempfile::TempDir;

use crate::models::install::{BinaryKind, LinuxArch};
use crate::models::release::{Release, ReleaseAsset};
use crate::services::github::GitHubClient;
use crate::services::storage::{InstallLayout, detect_linux_arch};

#[derive(Clone, Debug)]
pub struct PreviaManager {
    github: GitHubClient,
    layout: InstallLayout,
    arch: LinuxArch,
}

impl PreviaManager {
    pub fn new() -> Result<Self> {
        Ok(Self {
            github: GitHubClient::new()?,
            layout: InstallLayout::from_home()?,
            arch: detect_linux_arch()?,
        })
    }

    pub async fn install_latest(&self) -> Result<()> {
        self.layout.ensure_dirs()?;

        let release = self.github.latest_release().await?;
        let latest_tag = sanitize_tag(&release.tag);
        let current = self.layout.current_version()?;

        self.ensure_release_installed(&release).await?;

        if current.is_none() {
            self.layout.set_current(&latest_tag)?;
            self.layout.refresh_binary_links()?;
            println!(
                "instalacao concluida e versao ativa definida para {}",
                latest_tag
            );
            println!("binarios ativos em {:?}", self.layout.managed_bin_dir);
            return Ok(());
        }

        let current = current.expect("checked is_some above");
        if current.tag == latest_tag {
            println!("{} ja esta ativa.", latest_tag);
            println!("nenhuma alteracao foi feita.");
            return Ok(());
        }

        println!("instalacao concluida: {}", latest_tag);
        println!("versao ativa mantida: {}", current.tag);
        println!("use `previactl update` para trocar a versao ativa quando quiser.");
        Ok(())
    }

    pub async fn update(&self) -> Result<()> {
        self.layout.ensure_dirs()?;

        let release = self.github.latest_release().await?;
        let latest_tag = sanitize_tag(&release.tag);
        let current = self.layout.current_version()?;

        let Some(current) = current else {
            println!("nenhuma versao ativa encontrada. executando instalacao inicial.");
            self.ensure_release_installed(&release).await?;
            self.layout.set_current(&latest_tag)?;
            self.layout.refresh_binary_links()?;
            println!("versao ativa definida para {}", latest_tag);
            return Ok(());
        };

        match compare_versions(&latest_tag, &current.tag) {
            Ordering::Greater => {
                println!(
                    "nova versao disponivel: {} (atual: {})",
                    latest_tag, current.tag
                );
            }
            Ordering::Equal => {
                println!("ja esta atualizado. versao ativa: {}", current.tag);
                return Ok(());
            }
            Ordering::Less => {
                println!(
                    "versao ativa ({}) e mais nova que release latest ({}).",
                    current.tag, latest_tag
                );
                return Ok(());
            }
        }

        let should_update = Confirm::new()
            .with_prompt(format!("atualizar para {}?", latest_tag))
            .default(true)
            .interact()
            .context("falha ao ler confirmacao de atualizacao")?;

        if !should_update {
            println!("atualizacao cancelada pelo usuario.");
            return Ok(());
        }

        self.ensure_release_installed(&release).await?;
        self.layout.set_current(&latest_tag)?;
        self.layout.refresh_binary_links()?;

        println!("atualizacao concluida. versao ativa: {}", latest_tag);
        Ok(())
    }

    pub fn uninstall(&self) -> Result<()> {
        self.layout.remove_managed_artifacts()?;
        println!("binarios gerenciados removidos: previa-main e previa-runner");
        Ok(())
    }

    async fn ensure_release_installed(&self, release: &Release) -> Result<()> {
        let tag = sanitize_tag(&release.tag);
        let version_dir = self.layout.version_dir(&tag);

        if self.layout.has_version(&tag) {
            println!("versao {} ja esta instalada em {:?}", tag, version_dir);
            return Ok(());
        }

        fs::create_dir_all(&version_dir)
            .with_context(|| format!("falha ao criar diretorio de versao {:?}", version_dir))?;

        let temp_dir = TempDir::new().context("falha ao criar diretorio temporario")?;

        for kind in BinaryKind::all() {
            let asset = self.resolve_asset(release, kind)?;
            let downloaded = temp_dir.path().join(&asset.name);

            println!("baixando {}: {}", kind.file_name(), asset.name);
            self.github
                .download_to_path(&asset.download_url, &downloaded)
                .await?;

            let destination = self.layout.version_binary_path(&tag, kind);
            self.install_binary_from_asset(kind, &downloaded, &asset.name, &destination)?;
            println!("instalado {} em {:?}", kind.file_name(), destination);
        }

        Ok(())
    }

    fn resolve_asset<'a>(
        &self,
        release: &'a Release,
        kind: BinaryKind,
    ) -> Result<&'a ReleaseAsset> {
        let base = kind.file_name();
        let linux_arch = self.arch.slug;
        let linux_alt = self.arch.alt;
        let target = self.arch.target_triple;

        let exact_candidates = [
            format!("{base}-linux-{linux_arch}"),
            format!("{base}-linux-{linux_alt}"),
            format!("{base}-{target}"),
        ];

        for candidate in exact_candidates {
            for ext in ["", ".tar.gz", ".tgz", ".zip", ".gz"] {
                let name = format!("{candidate}{ext}");
                if let Some(asset) = release.assets.iter().find(|asset| asset.name == name) {
                    return Ok(asset);
                }
            }
        }

        if let Some(asset) = release.assets.iter().find(|asset| {
            let name = asset.name.to_ascii_lowercase();
            name.contains(base)
                && name.contains("linux")
                && (name.contains(linux_arch) || name.contains(linux_alt) || name.contains(target))
        }) {
            return Ok(asset);
        }

        if let Some(asset) = release
            .assets
            .iter()
            .find(|asset| asset.name.to_ascii_lowercase().contains(base))
        {
            return Ok(asset);
        }

        Err(anyhow!(
            "asset nao encontrado para {} na release {}",
            kind.label(),
            release.tag
        ))
    }

    fn install_binary_from_asset(
        &self,
        kind: BinaryKind,
        downloaded: &std::path::Path,
        asset_name: &str,
        destination: &std::path::Path,
    ) -> Result<()> {
        if asset_name.ends_with(".tar.gz") || asset_name.ends_with(".tgz") {
            self.extract_from_tar_gz(kind, downloaded, destination)?;
        } else if asset_name.ends_with(".zip") {
            self.extract_from_zip(kind, downloaded, destination)?;
        } else if asset_name.ends_with(".gz") {
            self.extract_from_gz(downloaded, destination)?;
        } else {
            fs::copy(downloaded, destination).with_context(|| {
                format!(
                    "falha ao copiar binario de {:?} para {:?}",
                    downloaded, destination
                )
            })?;
            self.make_executable(destination)?;
        }

        Ok(())
    }

    fn extract_from_tar_gz(
        &self,
        kind: BinaryKind,
        archive_path: &std::path::Path,
        destination: &std::path::Path,
    ) -> Result<()> {
        let file = File::open(archive_path)
            .with_context(|| format!("falha ao abrir arquivo {:?}", archive_path))?;
        let decoder = flate2::read::GzDecoder::new(file);
        let mut archive = Archive::new(decoder);

        let mut found = false;
        for entry_result in archive.entries().context("falha ao ler entries do tar")? {
            let mut entry = entry_result.context("falha ao ler entry do tar")?;
            if !entry.header().entry_type().is_file() {
                continue;
            }

            let path = entry.path().context("falha ao ler caminho da entry")?;
            let Some(file_name) = path.file_name().and_then(|name| name.to_str()) else {
                continue;
            };

            if file_name == kind.file_name() || file_name.starts_with(kind.file_name()) {
                let mut out = File::create(destination)
                    .with_context(|| format!("falha ao criar {:?}", destination))?;
                io::copy(&mut entry, &mut out)
                    .with_context(|| format!("falha ao extrair entry para {:?}", destination))?;
                found = true;
                break;
            }
        }

        if !found {
            return Err(anyhow!(
                "binario {} nao encontrado no tar {:?}",
                kind.file_name(),
                archive_path
            ));
        }

        self.make_executable(destination)
    }

    fn extract_from_zip(
        &self,
        kind: BinaryKind,
        archive_path: &std::path::Path,
        destination: &std::path::Path,
    ) -> Result<()> {
        let file = File::open(archive_path)
            .with_context(|| format!("falha ao abrir arquivo {:?}", archive_path))?;
        let mut archive = zip::ZipArchive::new(file).context("falha ao ler zip")?;

        for idx in 0..archive.len() {
            let mut entry = archive.by_index(idx).context("falha ao ler entry zip")?;
            if entry.is_dir() {
                continue;
            }

            let name = entry.name().to_string();
            let Some(file_name) = std::path::Path::new(&name)
                .file_name()
                .and_then(|value| value.to_str())
            else {
                continue;
            };

            if file_name == kind.file_name() || file_name.starts_with(kind.file_name()) {
                let mut out = File::create(destination)
                    .with_context(|| format!("falha ao criar {:?}", destination))?;
                io::copy(&mut entry, &mut out)
                    .with_context(|| format!("falha ao extrair entry para {:?}", destination))?;
                self.make_executable(destination)?;
                return Ok(());
            }
        }

        Err(anyhow!(
            "binario {} nao encontrado no zip {:?}",
            kind.file_name(),
            archive_path
        ))
    }

    fn extract_from_gz(
        &self,
        archive_path: &std::path::Path,
        destination: &std::path::Path,
    ) -> Result<()> {
        let file = File::open(archive_path)
            .with_context(|| format!("falha ao abrir arquivo {:?}", archive_path))?;
        let mut decoder = flate2::read::GzDecoder::new(file);
        let mut out = File::create(destination)
            .with_context(|| format!("falha ao criar {:?}", destination))?;
        io::copy(&mut decoder, &mut out)
            .with_context(|| format!("falha ao descompactar gz para {:?}", destination))?;

        self.make_executable(destination)
    }

    fn make_executable(&self, destination: &std::path::Path) -> Result<()> {
        let mut perm = fs::metadata(destination)
            .with_context(|| format!("falha ao ler metadados de {:?}", destination))?
            .permissions();
        perm.set_mode(0o755);
        fs::set_permissions(destination, perm).with_context(|| {
            format!("falha ao definir permissao executavel em {:?}", destination)
        })?;
        Ok(())
    }
}

fn sanitize_tag(tag: &str) -> String {
    let trimmed = tag.trim();
    if trimmed.is_empty() {
        return "unknown".to_string();
    }

    if trimmed.starts_with('v') {
        trimmed.to_string()
    } else {
        format!("v{trimmed}")
    }
}

fn compare_versions(latest: &str, current: &str) -> Ordering {
    let parsed_latest = Version::parse(latest.trim_start_matches('v'));
    let parsed_current = Version::parse(current.trim_start_matches('v'));

    match (parsed_latest, parsed_current) {
        (Ok(latest_version), Ok(current_version)) => latest_version.cmp(&current_version),
        _ => latest.cmp(current),
    }
}

#[cfg(test)]
mod tests {
    use super::{compare_versions, sanitize_tag};

    #[test]
    fn sanitize_tag_adds_prefix() {
        assert_eq!(sanitize_tag("0.3.0"), "v0.3.0");
        assert_eq!(sanitize_tag("v0.3.0"), "v0.3.0");
    }

    #[test]
    fn compare_versions_uses_semver_when_possible() {
        assert!(compare_versions("v1.10.0", "v1.9.9").is_gt());
        assert!(compare_versions("v1.9.9", "v1.10.0").is_lt());
        assert!(compare_versions("v1.0.0", "v1.0.0").is_eq());
    }
}
