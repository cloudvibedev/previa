# `previactl` v1 Specification

## Summary

`previactl` is the local operations CLI for Previa. Version 1 is Linux-first and
local-only: it installs, updates, uninstalls, runs, and manages
`previa-main` and `previa-runner` on the current host.

This document is implementation-ready. Anything not defined here is out of
scope for v1 and must not be invented during implementation.

## Product Goals

- Install `previa-main` and `previa-runner` from `https://files.previa.dev/manifest.json`.
- Detect the current operating system and CPU architecture and pick the correct
  artifact links from the manifest.
- Persist installation state without depending on `previa-main --version` or
  `previa-runner --version`.
- Run `previa-main` and `previa-runner` in foreground for local operations.
- Manage `previa-main` and `previa-runner` as `systemd` services on Linux.
- Reuse the current environment-variable contract already supported by the
  binaries.

## Non-Goals

- Remote provisioning over SSH.
- Fleet or cluster management across multiple hosts.
- Automatic runner registration in external control planes.
- `launchd` service management on macOS.
- Windows Service Manager integration.
- Checksum or signature verification before the manifest exposes those fields.

## External Contract

### Manifest Endpoint

- URL: `GET https://files.previa.dev/manifest.json`
- Content-Type: JSON
- Expected top-level schema:
  - `name: string`
  - `version: string`
  - `create_at: string` in RFC 3339 UTC format
  - `links: object<string, string>`

Example:

```json
{
  "name": "previa",
  "version": "0.0.5",
  "create_at": "2026-03-11T15:53:30Z",
  "links": {
    "previa_main_linux_amd64": "https://files.previa.dev/0.0.5/files/previa-main-linux-amd64",
    "previa_runner_linux_amd64": "https://files.previa.dev/0.0.5/files/previa-runner-linux-amd64"
  }
}
```

### Platform Mapping

The CLI must map the detected platform to manifest keys exactly as follows:

| OS | Architecture | `previa-main` key | `previa-runner` key |
| --- | --- | --- | --- |
| `linux` | `x86_64` | `previa_main_linux_amd64` | `previa_runner_linux_amd64` |
| `linux` | `aarch64` | `previa_main_linux_arm64` | `previa_runner_linux_arm64` |
| `macos` | `x86_64` | `previa_main_macos_amd64` | `previa_runner_macos_amd64` |
| `macos` | `aarch64` | `previa_main_macos_arm64` | `previa_runner_macos_arm64` |
| `windows` | `x86_64` | `previa_main_windows_amd64` | `previa_runner_windows_amd64` |

If the current `(os, arch)` pair is not in this table, `previactl` must fail
with an explicit unsupported-platform error before any download begins.

If the pair exists in the table but either manifest key is missing,
`previactl` must fail with an explicit missing-artifact error and print the
missing key name.

## Command Surface

The v1 CLI surface is fixed to the commands below:

```text
previactl install [--force-config]
previactl update
previactl uninstall [--purge]
previactl run main
previactl run runner
previactl service install main
previactl service install runner
previactl service start main
previactl service start runner
previactl service stop main
previactl service stop runner
previactl service restart main
previactl service restart runner
previactl service status main
previactl service status runner
previactl manifest show
```

No additional v1 commands are required.

### Command Semantics

#### `previactl manifest show`

- Fetches the remote manifest.
- Prints the parsed JSON in a human-readable format.
- Does not write to disk.

#### `previactl install [--force-config]`

- Fetches the remote manifest.
- Resolves the current platform.
- Downloads both `previa-main` and `previa-runner`.
- Installs the binaries into the configured installation paths.
- Creates base config files only if they do not already exist.
- If `--force-config` is provided, rewrites base config files even when they
  already exist.
- Writes installation metadata to `install-state.json`.
- Does not create or start system services automatically.

#### `previactl update`

- Fetches the remote manifest.
- Reads `install-state.json`.
- Compares the remote manifest version to the installed version string.
- If versions are equal, prints `already up to date` and exits successfully.
- If the remote version is newer, downloads both binaries and replaces them
  atomically.
- If either service is already installed and active, restarts that service after
  the binary swap completes.
- Does not overwrite existing config files.
- Updates `install-state.json` after both binaries are replaced successfully.

#### `previactl uninstall [--purge]`

- Stops and removes `previa-main.service` and `previa-runner.service` if they
  exist.
- Removes installed binaries and `install-state.json`.
- Without `--purge`, preserves `/etc/previa` and `/var/lib/previa`.
- With `--purge`, also removes `/etc/previa` and `/var/lib/previa`.

#### `previactl run main`

- Loads `/etc/previa/main.env`.
- Starts `/opt/previa/bin/previa-main` in foreground.
- Inherits stdout and stderr in the current terminal session.

#### `previactl run runner`

- Loads `/etc/previa/runner.env`.
- Starts `/opt/previa/bin/previa-runner` in foreground.
- Inherits stdout and stderr in the current terminal session.

#### `previactl service install main|runner`

- Creates the corresponding `systemd` unit file if it does not exist.
- Rewrites the unit file if it already exists to keep the generated definition
  authoritative.
- Runs `systemctl daemon-reload`.
- Does not start the service automatically.

#### `previactl service start|stop|restart|status main|runner`

- Proxies to `systemctl` for the respective unit name.
- Fails clearly on non-Linux platforms with `service management is only
  supported on linux`.

## Installation Layout

The Linux installation layout is fixed for v1:

- Binaries:
  - `/opt/previa/bin/previa-main`
  - `/opt/previa/bin/previa-runner`
- Config:
  - `/etc/previa/main.env`
  - `/etc/previa/runner.env`
- State:
  - `/var/lib/previa/install-state.json`
- Default orchestrator database:
  - `/var/lib/previa/main/orchestrator.db`
- `systemd` units:
  - `/etc/systemd/system/previa-main.service`
  - `/etc/systemd/system/previa-runner.service`

The installer must create parent directories as needed.

## Persistent State

### Install State File

Path: `/var/lib/previa/install-state.json`

Schema:

```json
{
  "name": "previa",
  "version": "0.0.5",
  "platform": {
    "os": "linux",
    "arch": "x86_64"
  },
  "installed_at": "2026-03-11T16:10:00Z",
  "artifacts": {
    "main": {
      "source_url": "https://files.previa.dev/0.0.5/files/previa-main-linux-amd64",
      "path": "/opt/previa/bin/previa-main"
    },
    "runner": {
      "source_url": "https://files.previa.dev/0.0.5/files/previa-runner-linux-amd64",
      "path": "/opt/previa/bin/previa-runner"
    }
  }
}
```

Rules:

- `version` is the source of truth for installed version checks.
- The file is written only after both binaries are installed successfully.
- The file is removed by `uninstall`.
- Partial writes must be avoided by writing to a temporary file in the same
  directory and renaming it into place.

## Configuration Model

`previactl` must reuse the environment variables already supported by the
existing binaries.

### `main.env`

Path: `/etc/previa/main.env`

Default content:

```dotenv
ADDRESS=0.0.0.0
PORT=5588
ORCHESTRATOR_DATABASE_URL=sqlite:///var/lib/previa/main/orchestrator.db
RUNNER_ENDPOINTS=http://127.0.0.1:55880
RUST_LOG=info
```

Notes:

- `ORCHESTRATOR_DATABASE_URL` must use the absolute path shown above.
- `RUNNER_ENDPOINTS` defaults to the local runner service started by the same
  host.
- If the file already exists, `install` and `update` must leave it unchanged
  unless `install --force-config` is used.

### `runner.env`

Path: `/etc/previa/runner.env`

Default content:

```dotenv
ADDRESS=0.0.0.0
PORT=55880
RUST_LOG=info
```

Notes:

- If the file already exists, `install` and `update` must leave it unchanged
  unless `install --force-config` is used.

## Linux `systemd` Integration

`systemd` management is supported only on Linux in v1.

### `previa-main.service`

```ini
[Unit]
Description=Previa Main
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
EnvironmentFile=/etc/previa/main.env
ExecStart=/opt/previa/bin/previa-main
WorkingDirectory=/var/lib/previa/main
Restart=on-failure
RestartSec=5

[Install]
WantedBy=multi-user.target
```

### `previa-runner.service`

```ini
[Unit]
Description=Previa Runner
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
EnvironmentFile=/etc/previa/runner.env
ExecStart=/opt/previa/bin/previa-runner
WorkingDirectory=/var/lib/previa
Restart=on-failure
RestartSec=5

[Install]
WantedBy=multi-user.target
```

Rules:

- `service install` writes these definitions exactly, with the paths above.
- The CLI must not enable the services automatically in v1.
- Logs are handled by `journald`; no extra log file configuration is introduced.

## Installation and Update Flow

### Download Rules

- Downloads must use the URLs from the manifest without rewriting them.
- Each binary is first downloaded to a temporary file in the destination
  directory.
- Temporary files are marked executable before the final rename.
- Final replacement uses atomic rename within the same filesystem.
- If any download or rename fails, existing installed binaries must remain in
  place.

### Install Flow

1. Fetch and parse the manifest.
2. Validate `name == "previa"`.
3. Detect platform and resolve the two artifact URLs.
4. Create required directories.
5. Download and atomically install both binaries.
6. Create default config files if absent, or rewrite them only with
   `--force-config`.
7. Write `install-state.json`.
8. Exit without touching service enabled/active state.

### Update Flow

1. Fetch and parse the manifest.
2. Read `install-state.json`; if missing, fail with `not installed`.
3. Compare remote `version` to installed `version`.
4. If equal, print `already up to date` and exit with code `0`.
5. Resolve current platform URLs from the remote manifest.
6. Download and atomically replace both binaries.
7. Rewrite `install-state.json` with the new version and artifact URLs.
8. Restart `previa-main.service` and `previa-runner.service` only if each
   service both exists and is currently active.

## Error Handling

The implementation must surface explicit user-facing errors for:

- Unsupported operating system.
- Unsupported CPU architecture.
- Missing manifest keys for the current platform.
- Invalid or incomplete manifest schema.
- HTTP download failures.
- Missing installation state during `update`.
- Missing installed binary during `run`.
- Non-Linux `service` command usage.
- Permission failures when writing to `/opt`, `/etc`, `/var/lib`, or
  `/etc/systemd/system`.

## Test Plan

The implementation is complete only when these scenarios are covered:

1. Clean install on Linux `x86_64` with a valid manifest installs both binaries
   and writes `install-state.json`.
2. Clean install on Linux `aarch64` resolves the `_arm64` manifest keys.
3. `update` with equal local and remote versions prints `already up to date`.
4. `update` with a newer remote version replaces both binaries and updates
   `install-state.json`.
5. Missing manifest key for the detected platform fails before any binary is
   replaced.
6. Failed download of either binary leaves the existing installation untouched.
7. `service install main` writes the expected unit file with
   `EnvironmentFile=/etc/previa/main.env`.
8. `service install runner` writes the expected unit file with
   `EnvironmentFile=/etc/previa/runner.env`.
9. `run main` starts `previa-main` with
   `ORCHESTRATOR_DATABASE_URL=sqlite:///var/lib/previa/main/orchestrator.db`
   when the default config is used.
10. `run runner` starts `previa-runner` with default `ADDRESS=0.0.0.0` and
    `PORT=55880` when the default config is used.
11. `uninstall` without `--purge` removes binaries and units but preserves
    `/etc/previa` and `/var/lib/previa`.
12. Reinstall after non-purge uninstall reuses the preserved config files.
13. `service` commands on macOS or Windows fail with the documented Linux-only
    service-management error.

## Rollback and Recovery

- Automatic rollback is out of scope for v1.
- If `update` fails before atomic rename, the previous installation remains
  authoritative.
- If `update` fails after one service restart and before the second restart, the
  operator must recover manually by rerunning `previactl update` or restoring
  the previous binaries.

## Security and Known Risks

- The manifest is trusted as the release source of truth.
- No checksum verification is available in v1 because the current manifest does
  not expose checksum fields.
- No signature verification is available in v1.
- Adding checksums and signed release verification is mandatory hardening work
  for v2.

## Implementation Notes

- The future crate will be named `previactl`.
- It should remain separate from HTTP transport concerns and reuse dedicated
  modules for manifest fetching, platform detection, installation state,
  service management, and process execution.
- The CLI must target the existing `previa-main` and `previa-runner` contracts
  without requiring changes to those binaries for v1.
