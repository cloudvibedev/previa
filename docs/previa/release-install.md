# Release and Install

This guide explains how Previa binaries and Docker images are published, and how installation works from the operator point of view.

## Installer

The public installer is:

```bash
curl -fsSL https://downloads.previa.dev/install.sh | sh
```

It currently:

- downloads `latest.json` from `https://downloads.previa.dev/latest.json`
- resolves the latest published version and links
- detects Linux or macOS before choosing the `previa` control binary to install
- installs the `previa` CLI under `~/.previa/bin`
- sets `PREVIA_HOME="$HOME/.previa"`
- updates `~/.zshrc` and `~/.bashrc` when they exist

## What Gets Installed

The installer installs the `previa` control binary.

Published Linux binaries are built against `musl` so they stay portable across a wider range of Linux distributions and do not depend on a very recent host `glibc`.

On macOS, the installer resolves the `previa` control binary for macOS. When the manifest does not yet expose a direct macOS download link, the installer falls back to the matching GitHub Release asset for the resolved version.

At runtime, `previa up --bin` can also fetch missing runtime binaries such as:

- `previa-main`
- `previa-runner`

Those are installed under:

```text
$PREVIA_HOME/bin
```

When `previa up --bin` downloads those runtime binaries, it targets the exact same release version as the running `previa` CLI.

## Compatibility

Previa keeps local runtime pieces aligned by version:

- `previa up` uses the current CLI version tag by default for Docker-backed runtimes
- `previa pull` uses the current CLI version tag by default
- `previa up --bin` downloads `previa-main` and `previa-runner` for the exact current CLI version

That means the default behavior is:

```text
previa == previa-main == previa-runner
```

unless you explicitly override the image tag for a compose-backed runtime with `--version`.

## Uninstall

To remove a default installation, delete `~/.previa` and remove the installer block from your shell rc files.

Remove the default home:

```bash
rm -rf ~/.previa
```

If you installed Previa into a custom home, remove that directory instead:

```bash
rm -rf "$PREVIA_HOME"
```

Then remove the lines added by the installer between:

```text
# >>> Previa installer >>>
# <<< Previa installer <<<
```

The installer may write that block to:

- `~/.zshrc`
- `~/.bashrc`

After removing the block, open a new shell or reload your rc file so `PATH` no longer includes `PREVIA_HOME/bin`.

## `latest.json`

The release workflow publishes a manifest at:

```text
https://downloads.previa.dev/latest.json
```

That manifest contains:

- the latest version
- direct download links for published binaries

Example link keys include:

- `previa_linux_amd64`
- `previa_main_linux_amd64`
- `previa_runner_linux_amd64`

## Docker Images

The release workflow also publishes Docker images to GHCR:

- `ghcr.io/cloudvibedev/main`
- `ghcr.io/cloudvibedev/runner`

These are the images used by compose-backed runtime flows.

## Release Workflow

At a high level, the release workflow:

1. resolves the workspace version
2. builds binaries for the selected release scope
3. creates a git tag and GitHub release
4. optionally uploads Linux binaries to Cloudflare R2
5. optionally publishes `latest.json`
6. optionally publishes `install.sh`
7. optionally builds and pushes Docker images to GHCR
8. optionally publishes runtime crates to crates.io

The workflow dispatch now accepts a `release_scope` choice:

- `linux`: publishes Linux release assets and also runs Docker, crates.io, and bucket/R2 publishing
- `mac`: publishes only the macOS release asset
- `windows`: publishes only the Windows release asset
- `all`: publishes Linux release assets plus Docker, crates.io, bucket/R2, and also adds macOS and Windows release assets

Current platform coverage:

- Linux:
  - `previa`
  - `previa-main`
  - `previa-runner`
  - `amd64` and `arm64`
- macOS:
  - `previa`
  - `amd64`
- Windows:
  - `previa.exe`
  - `amd64`

Only Linux artifacts are uploaded to Cloudflare R2 and included in `latest.json`, because the installer and runtime binary download flow remain Linux-centric in this release model.

## Practical Notes

- compose-backed mode uses published Docker images
- binary-backed mode uses local binaries and can auto-download missing runtime binaries
- the default path keeps CLI and runtime versions aligned
- published runtime binaries currently target Linux
- macOS and Windows release assets currently ship only the control binary `previa`

## See Also

- [Runtime modes](./runtime-modes.md)
- [Minimal happy path](./minimal-happy-path.md)
