# Project Notes

## Release and Install Workflow

- Keep GitHub Release asset names aligned with installer platform slugs:
  - Linux: `previa-linux-amd64`, `previa-linux-arm64`
  - macOS: `previa-macos-amd64`, `previa-macos-arm64`
  - Windows: `previa-windows-amd64.exe`
- Keep `scripts/generate_release_metadata.py` in sync with `.github/workflows/release.yaml` whenever release matrix entries change.
- Keep `install.sh` architecture detection aligned with published Unix release assets.
- After release workflow or installer changes, validate:
  - `sh -n install.sh`
  - `python3 scripts/test_release_metadata.py`
  - `cargo build --release`
  - `cargo test`
