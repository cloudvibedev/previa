use std::process::Stdio;

use anyhow::{Context, Result, bail};
use tokio::process::Command;

use crate::cli::PullTarget;

pub const MAIN_IMAGE_REPOSITORY: &str = "ghcr.io/cloudvibedev/main";
pub const RUNNER_IMAGE_REPOSITORY: &str = "ghcr.io/cloudvibedev/runner";

pub fn normalize_image_tag(version: &str) -> Result<String> {
    let version = version.trim();
    if version.is_empty() {
        bail!("--version cannot be empty");
    }
    Ok(version.to_owned())
}

pub fn main_image_ref(version: &str) -> Result<String> {
    Ok(format!(
        "{MAIN_IMAGE_REPOSITORY}:{}",
        normalize_image_tag(version)?
    ))
}

pub fn runner_image_ref(version: &str) -> Result<String> {
    Ok(format!(
        "{RUNNER_IMAGE_REPOSITORY}:{}",
        normalize_image_tag(version)?
    ))
}

pub fn resolve_image_refs(target: PullTarget, version: &str) -> Result<Vec<String>> {
    let refs = match target {
        PullTarget::Main => vec![main_image_ref(version)?],
        PullTarget::Runner => vec![runner_image_ref(version)?],
        PullTarget::All => vec![main_image_ref(version)?, runner_image_ref(version)?],
    };
    Ok(refs)
}

pub async fn pull_images(target: PullTarget, version: &str) -> Result<()> {
    for image_ref in resolve_image_refs(target, version)? {
        println!("pulling {image_ref}");

        let status = Command::new("docker")
            .arg("pull")
            .arg(&image_ref)
            .stdin(Stdio::null())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .status()
            .await
            .with_context(
                || "failed to spawn 'docker'; ensure Docker CLI is installed and available in PATH",
            )?;

        if !status.success() {
            bail!("docker pull failed for '{image_ref}' with status {status}");
        }

        println!("pulled {image_ref}");
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::resolve_image_refs;
    use crate::cli::PullTarget;

    #[test]
    fn resolves_latest_refs_for_all() {
        let refs = resolve_image_refs(PullTarget::All, "latest").expect("refs");
        assert_eq!(
            refs,
            vec![
                "ghcr.io/cloudvibedev/main:latest",
                "ghcr.io/cloudvibedev/runner:latest",
            ]
        );
    }

    #[test]
    fn resolves_specific_version_for_single_target() {
        let refs = resolve_image_refs(PullTarget::Runner, "0.0.7").expect("refs");
        assert_eq!(refs, vec!["ghcr.io/cloudvibedev/runner:0.0.7"]);
    }

    #[test]
    fn rejects_empty_version() {
        let err = resolve_image_refs(PullTarget::Main, "   ").expect_err("error");
        assert!(err.to_string().contains("--version cannot be empty"));
    }
}
