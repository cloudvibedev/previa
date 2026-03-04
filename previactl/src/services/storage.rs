use std::fs;
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow};

use crate::models::install::{BinaryKind, InstalledVersion, LinuxArch};

const PREVIA_DIR: &str = ".local/share/previa";
const PREVIA_BIN_DIR: &str = ".local/share/previa/bin";
const USER_BIN_DIR: &str = ".local/bin";

#[derive(Clone, Debug)]
pub struct InstallLayout {
    pub root_dir: PathBuf,
    pub versions_dir: PathBuf,
    pub current_link: PathBuf,
    pub managed_bin_dir: PathBuf,
    pub user_bin_dir: PathBuf,
}

impl InstallLayout {
    pub fn from_home() -> Result<Self> {
        let home = std::env::var_os("HOME")
            .map(PathBuf::from)
            .ok_or_else(|| anyhow!("variavel HOME nao encontrada"))?;

        let root_dir = home.join(PREVIA_DIR);
        let versions_dir = root_dir.join("versions");
        let current_link = root_dir.join("current");
        let managed_bin_dir = home.join(PREVIA_BIN_DIR);
        let user_bin_dir = home.join(USER_BIN_DIR);

        Ok(Self {
            root_dir,
            versions_dir,
            current_link,
            managed_bin_dir,
            user_bin_dir,
        })
    }

    pub fn ensure_dirs(&self) -> Result<()> {
        fs::create_dir_all(&self.versions_dir)
            .with_context(|| format!("falha ao criar {:?}", self.versions_dir))?;
        fs::create_dir_all(&self.managed_bin_dir)
            .with_context(|| format!("falha ao criar {:?}", self.managed_bin_dir))?;
        fs::create_dir_all(&self.user_bin_dir)
            .with_context(|| format!("falha ao criar {:?}", self.user_bin_dir))?;
        Ok(())
    }

    pub fn version_dir(&self, tag: &str) -> PathBuf {
        self.versions_dir.join(tag)
    }

    pub fn version_binary_path(&self, tag: &str, kind: BinaryKind) -> PathBuf {
        self.version_dir(tag).join(kind.file_name())
    }

    pub fn managed_binary_path(&self, kind: BinaryKind) -> PathBuf {
        self.managed_bin_dir.join(kind.file_name())
    }

    pub fn user_binary_path(&self, kind: BinaryKind) -> PathBuf {
        self.user_bin_dir.join(kind.file_name())
    }

    pub fn has_version(&self, tag: &str) -> bool {
        BinaryKind::all()
            .iter()
            .all(|kind| self.version_binary_path(tag, *kind).is_file())
    }

    pub fn current_version(&self) -> Result<Option<InstalledVersion>> {
        if !self.current_link.exists() {
            return Ok(None);
        }

        let linked = fs::read_link(&self.current_link)
            .with_context(|| format!("falha ao ler link {:?}", self.current_link))?;

        let resolved = if linked.is_absolute() {
            linked
        } else {
            self.root_dir.join(linked)
        };

        let tag = resolved
            .file_name()
            .and_then(|name| name.to_str())
            .map(str::to_owned)
            .ok_or_else(|| anyhow!("link current invalido: {:?}", resolved))?;

        Ok(Some(InstalledVersion {
            tag,
            path: resolved,
        }))
    }

    pub fn set_current(&self, tag: &str) -> Result<()> {
        let target_dir = self.version_dir(tag);
        if !target_dir.is_dir() {
            return Err(anyhow!("versao {} nao encontrada em {:?}", tag, target_dir));
        }

        let tmp_link = self.root_dir.join("current.tmp");
        if tmp_link.exists() {
            fs::remove_file(&tmp_link)
                .with_context(|| format!("falha ao remover {:?}", tmp_link))?;
        }

        symlink(&target_dir, &tmp_link)
            .with_context(|| format!("falha ao criar link temporario {:?}", tmp_link))?;

        if self.current_link.exists() {
            fs::remove_file(&self.current_link)
                .with_context(|| format!("falha ao remover {:?}", self.current_link))?;
        }

        fs::rename(&tmp_link, &self.current_link)
            .with_context(|| format!("falha ao atualizar link {:?}", self.current_link))?;

        Ok(())
    }

    pub fn refresh_binary_links(&self) -> Result<()> {
        let current = self
            .current_version()?
            .ok_or_else(|| anyhow!("nenhuma versao ativa para criar links"))?;

        for kind in BinaryKind::all() {
            let current_binary = current.path.join(kind.file_name());
            if !current_binary.is_file() {
                return Err(anyhow!("binario ausente: {:?}", current_binary));
            }

            let managed = self.managed_binary_path(kind);
            self.relink(&managed, &current_binary)?;

            let user = self.user_binary_path(kind);
            self.relink_user_binary(&user, &managed)?;
        }

        Ok(())
    }

    pub fn remove_managed_artifacts(&self) -> Result<()> {
        if self.root_dir.exists() {
            fs::remove_dir_all(&self.root_dir)
                .with_context(|| format!("falha ao remover {:?}", self.root_dir))?;
        }

        for kind in BinaryKind::all() {
            let user_path = self.user_binary_path(kind);
            if let Ok(target) = fs::read_link(&user_path) {
                let absolute_target = if target.is_absolute() {
                    target
                } else {
                    user_path
                        .parent()
                        .map(Path::to_path_buf)
                        .unwrap_or_else(|| PathBuf::from("."))
                        .join(target)
                };

                if absolute_target.starts_with(&self.root_dir)
                    || absolute_target.starts_with(&self.managed_bin_dir)
                {
                    let _ = fs::remove_file(&user_path);
                }
            }
        }

        Ok(())
    }

    fn relink(&self, link_path: &Path, target: &Path) -> Result<()> {
        if link_path.exists() {
            fs::remove_file(link_path)
                .with_context(|| format!("falha ao remover link {:?}", link_path))?;
        }

        symlink(target, link_path)
            .with_context(|| format!("falha ao criar link {:?} -> {:?}", link_path, target))?;

        Ok(())
    }

    fn relink_user_binary(&self, user_link: &Path, target: &Path) -> Result<()> {
        if user_link.exists() {
            if let Ok(existing_target) = fs::read_link(user_link) {
                let existing_abs = if existing_target.is_absolute() {
                    existing_target
                } else {
                    user_link
                        .parent()
                        .map(Path::to_path_buf)
                        .unwrap_or_else(|| PathBuf::from("."))
                        .join(existing_target)
                };

                if existing_abs.starts_with(&self.root_dir)
                    || existing_abs.starts_with(&self.managed_bin_dir)
                {
                    fs::remove_file(user_link).with_context(|| {
                        format!("falha ao remover link existente {:?}", user_link)
                    })?;
                } else {
                    println!(
                        "aviso: {} ja existe em {:?} e nao e gerenciado pelo previactl; mantendo arquivo.",
                        user_link
                            .file_name()
                            .and_then(|name| name.to_str())
                            .unwrap_or("binario"),
                        user_link
                    );
                    return Ok(());
                }
            } else {
                println!(
                    "aviso: {} ja existe em {:?} e nao e link simbolico; mantendo arquivo.",
                    user_link
                        .file_name()
                        .and_then(|name| name.to_str())
                        .unwrap_or("binario"),
                    user_link
                );
                return Ok(());
            }
        }

        symlink(target, user_link)
            .with_context(|| format!("falha ao criar link {:?} -> {:?}", user_link, target))?;
        Ok(())
    }
}

pub fn detect_linux_arch() -> Result<LinuxArch> {
    let arch = std::env::consts::ARCH;
    match arch {
        "x86_64" | "amd64" => Ok(LinuxArch {
            slug: "amd64",
            alt: "x86_64",
            target_triple: "x86_64-unknown-linux-gnu",
        }),
        "aarch64" | "arm64" => Ok(LinuxArch {
            slug: "arm64",
            alt: "aarch64",
            target_triple: "aarch64-unknown-linux-gnu",
        }),
        _ => Err(anyhow!("arquitetura linux nao suportada: {}", arch)),
    }
}
