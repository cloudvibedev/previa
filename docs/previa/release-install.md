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
- installs the `previa` CLI under `~/.previa/bin`
- sets `PREVIA_HOME="$HOME/.previa"`
- updates `~/.zshrc` and `~/.bashrc` when they exist

## What Gets Installed

The installer installs the `previa` control binary.

Published Linux binaries are built against `musl` so they stay portable across a wider range of Linux distributions and do not depend on a very recent host `glibc`.

At runtime, `previa up --bin` can also fetch missing runtime binaries such as:

- `previa-main`
- `previa-runner`

Those are installed under:

```text
$PREVIA_HOME/bin
```

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
2. builds Linux binaries
3. uploads binaries to Cloudflare R2
4. publishes `latest.json`
5. publishes `install.sh`
6. creates a git tag and GitHub release
7. builds and pushes Docker images to GHCR

## Practical Notes

- compose-backed mode uses published Docker images
- binary-backed mode uses local binaries and can auto-download missing runtime binaries
- published runtime binaries currently target Linux

## See Also

- [Runtime modes](./runtime-modes.md)
- [Minimal happy path](./minimal-happy-path.md)
